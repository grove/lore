"""Bounded, content-free diagnostics for Guardian's source-integrity oracle.

This module supplements byte fingerprints; it never supplies expected hashes or
attributes a writer from a timestamp alone. Linux inotify observes kernel events
independently of the before/after inventory. It does not provide writer PIDs.
"""
from __future__ import annotations

import ctypes
import hashlib
import os
from pathlib import Path
import platform
import stat
import struct
import time

import cross_source as cross

MAX_PATHS = 4096
MAX_EVENTS = 20000


def file_state(metadata) -> tuple:
    """Comparable only within one stat API; Windows ctime bases can differ."""
    return (metadata.st_dev, metadata.st_ino, metadata.st_mode, metadata.st_size,
            metadata.st_mtime_ns, metadata.st_ctime_ns)


def category(relative: str) -> str:
    first = relative.split("/")[0]
    if first == ".lore":
        return "lore_state"
    if first == "wiki":
        return "generated_output"
    if relative == "lore.yml":
        return "configuration"
    return "primary_source"


def inventory(project: Path) -> dict:
    """Capture identity and hashes without following any symlink or reading text."""
    cross.reject_symlink_path(project)
    entries = {}
    total_bytes = 0
    pending = [project]
    while pending:
        directory = pending.pop()
        for path in sorted(directory.iterdir()):
            if len(entries) >= MAX_PATHS:
                raise ValueError("Integrity inventory exceeds its path bound")
            relative = path.relative_to(project).as_posix()
            metadata = path.lstat()
            kind = ("file" if stat.S_ISREG(metadata.st_mode) else "directory"
                    if stat.S_ISDIR(metadata.st_mode) else "symlink"
                    if stat.S_ISLNK(metadata.st_mode) else "nonregular")
            entry = {"kind": kind, "category": category(relative),
                     "device": metadata.st_dev, "inode": metadata.st_ino,
                     "mode": stat.S_IMODE(metadata.st_mode), "size": metadata.st_size,
                     "mtime_ns": metadata.st_mtime_ns, "ctime_ns": metadata.st_ctime_ns,
                     "descriptor_ctime_ns": None,
                     "sha256": None}
            if kind == "directory":
                pending.append(path)
            elif kind == "file":
                total_bytes += metadata.st_size
                if total_bytes > 128_000_000:
                    raise ValueError("Integrity inventory exceeds its byte bound")
                # O_NOFOLLOW protects the final component during the actual read.
                # Ancestor and concurrent-change checks fail this capture closed.
                cross.reject_symlink_path(path)
                descriptor = os.open(path, os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0) | getattr(os, "O_BINARY", 0))
                with os.fdopen(descriptor, "rb") as stream:
                    opened = os.fstat(stream.fileno())
                    if file_state(opened)[:-1] != file_state(metadata)[:-1]:
                        raise ValueError("A path changed identity during integrity capture")
                    digest = hashlib.sha256()
                    for block in iter(lambda: stream.read(65536), b""):
                        digest.update(block)
                    final = os.fstat(stream.fileno())
                # On Windows, lstat ctime can be creation time while fstat ctime
                # is metadata-change time (CPython issue 157671). Compare each
                # API to itself, retaining both timestamps rather than dropping
                # change detection or assigning a common invented timestamp.
                named_after = path.lstat()
                if file_state(final) != file_state(opened) or file_state(named_after) != file_state(metadata):
                    raise ValueError("A file changed during integrity capture")
                entry["descriptor_ctime_ns"] = final.st_ctime_ns
                entry["sha256"] = digest.hexdigest()
            entries[relative] = entry
    return {"sha256": cross.digest(entries), "paths": entries}


def effects(before: dict, after: dict) -> list[dict]:
    """Preserve create/delete/write/identity changes; infer only unambiguous renames."""
    original, current = before.get("paths", {}), after.get("paths", {})
    deleted, created = set(original) - current.keys(), set(current) - original.keys()
    result = []
    for source in sorted(deleted):
        old = original[source]
        matches = [target for target in created if old["kind"] == "file"
                   and (old["device"], old["inode"]) == (current[target]["device"], current[target]["inode"])]
        if len(matches) == 1:
            target = matches[0]
            created.remove(target)
            result.append({"operation": "rename_observed_identity", "path": source,
                           "destination": target, "category": category(source),
                           "destination_category": category(target), "before": old, "after": current[target]})
        else:
            result.append({"operation": "delete", "path": source, "category": category(source),
                           "before": old, "after": None})
    for target in sorted(created):
        result.append({"operation": "create", "path": target, "category": category(target),
                       "before": None, "after": current[target]})
    for path in sorted(original.keys() & current.keys()):
        old, new = original[path], current[path]
        if old != new:
            operation = ("replace" if (old["device"], old["inode"], old["kind"]) != (new["device"], new["inode"], new["kind"])
                         else "write" if old["sha256"] != new["sha256"] else "metadata_change")
            result.append({"operation": operation, "path": path, "category": category(path),
                           "before": old, "after": new})
    return result


class FilesystemObserver:
    """Explicit Linux inotify observer, independent of hash comparisons.

    Any overflow, unsupported platform, watch failure or newly created directory
    leaves completeness false. Inotify cannot attribute an event to its writer.
    """
    FLAGS = {0x2: "write", 0x4: "metadata_change", 0x8: "close_write",
             0x40: "rename_from", 0x80: "rename_to", 0x100: "create",
             0x200: "delete", 0x400: "delete_self", 0x800: "move_self"}

    def __init__(self, project: Path, requested: bool):
        self.project, self.requested = project, requested
        self.fd = None
        self.paths = {}
        self.events = []
        self.status = "not_requested"
        self.gaps = []
        self.started = time.monotonic()
        if not requested:
            return
        if platform.system() != "Linux":
            self.status = "unsupported_platform"
            return
        try:
            cross.reject_symlink_path(project)
            self.libc = ctypes.CDLL(None, use_errno=True)
            self.libc.inotify_init1.argtypes = [ctypes.c_int]
            self.libc.inotify_init1.restype = ctypes.c_int
            self.libc.inotify_add_watch.argtypes = [ctypes.c_int, ctypes.c_char_p, ctypes.c_uint32]
            self.libc.inotify_add_watch.restype = ctypes.c_int
            self.fd = self.libc.inotify_init1(os.O_NONBLOCK | os.O_CLOEXEC)
            if self.fd < 0:
                self.fd = None
                self.status = "unavailable"
                return
            self.status = "captured"
            self._watch(project)
            for path in sorted(project.rglob("*")):
                if path.is_symlink():
                    self.gaps.append("symlink_not_followed")
                elif path.is_dir():
                    self._watch(path)
        except (AttributeError, OSError, ValueError):
            self.status = "unavailable"
            self.gaps.append("observer_setup_failed")

    def _watch(self, path: Path):
        if len(self.paths) >= MAX_PATHS:
            self.gaps.append("watch_bound")
            return
        cross.reject_symlink_path(path)
        mask = sum(self.FLAGS) | 0x01000000 | 0x02000000  # ONLYDIR, DONT_FOLLOW
        descriptor = self.libc.inotify_add_watch(self.fd, os.fsencode(path), mask)
        if descriptor < 0:
            self.gaps.append("watch_failed")
        else:
            self.paths[descriptor] = path

    def poll(self):
        if self.fd is None:
            return
        while True:
            try:
                raw = os.read(self.fd, 65536)
            except BlockingIOError:
                break
            except OSError:
                self.gaps.append("observer_read_failed")
                break
            if not raw:
                break
            offset = 0
            while offset + 16 <= len(raw):
                descriptor, mask, cookie, length = struct.unpack_from("iIII", raw, offset)
                name = raw[offset + 16:offset + 16 + length].split(b"\0", 1)[0]
                offset += 16 + length
                if mask & 0x4000:
                    self.gaps.append("queue_overflow")
                    continue
                parent = self.paths.get(descriptor)
                if parent is None:
                    self.gaps.append("unknown_watch")
                    continue
                path = parent / os.fsdecode(name) if name else parent
                relative = path.relative_to(self.project).as_posix()
                if len(self.events) >= MAX_EVENTS:
                    self.gaps.append("event_bound")
                    continue
                operations = [value for flag, value in self.FLAGS.items() if mask & flag]
                if operations:
                    self.events.append({"path": relative, "category": category(relative),
                        "operations": operations, "rename_cookie": cookie or None,
                        "directory": bool(mask & 0x40000000),
                        "observed_seconds": round(time.monotonic() - self.started, 6), "writer_pid": None})
                if mask & 0x40000000 and mask & (0x100 | 0x80):
                    # A directory can receive writes before this watch is attached.
                    self.gaps.append(f"new_directory_watch_gap:{category(relative)}")
                    if path.is_dir() and not path.is_symlink():
                        self._watch(path)

    def finish(self) -> dict:
        self.poll()
        if self.fd is not None:
            os.close(self.fd)
            self.fd = None
        return {"kind": "linux_inotify", "requested": self.requested,
                "status": self.status, "complete": self.status == "captured" and not self.gaps,
                "source_complete": self.status == "captured" and all(gap in (
                    "new_directory_watch_gap:lore_state", "new_directory_watch_gap:generated_output") for gap in self.gaps),
                "gaps": sorted(set(self.gaps)), "events": self.events,
                "writer_attribution": "unavailable_inotify_has_no_writer_pid"}


def environment_classification(project: Path) -> dict:
    """No unrestricted environment, process arguments, paths or credentials."""
    try:
        fs_id = str(os.statvfs(project).f_fsid)
    except (AttributeError, OSError):
        fs_id = None
    return {"system": platform.system(), "python": platform.python_version(),
            "filesystem_identity_sha256": hashlib.sha256(fs_id.encode()).hexdigest() if fs_id else None,
            "workspace_mode": stat.S_IMODE(project.stat().st_mode),
            "workspace_path_sha256": hashlib.sha256(os.fsencode(project)).hexdigest(),
            "isolation": "exclusive_harness_directory_not_an_os_sandbox",
            "external_writers_excluded": None}


def process_tree(pid: int) -> dict:
    """Observe only the spawned process and its descendants; never inspect argv."""
    pending, entries, available = [pid], [], True
    while pending and len(entries) < 128:
        selected = pending.pop()
        try:
            fields = Path(f"/proc/{selected}/stat").read_text().rsplit(")", 1)[1].split()
            parent = int(fields[1])
            children = Path(f"/proc/{selected}/task/{selected}/children").read_text().split()
            pending.extend(int(child) for child in children)
            entries.append({"pid": selected, "parent_pid": parent})
        except (OSError, ValueError, IndexError):
            available = False
    return {"status": "observed" if available and entries and not pending else "unavailable_or_partial",
            "processes": entries, "arguments_captured": False}
