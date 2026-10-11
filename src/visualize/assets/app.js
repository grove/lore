"use strict";

/* Lore's browser never executes repository Markdown as JavaScript. The server
   renders a deliberately limited, escaped Markdown dialect; other text is
   always inserted using textContent. No external scripts or network calls. */
const $ = (id) => document.getElementById(id);
const state = {
  site: null, mode: "pages", selected: null, search: "", points: [], edges: [],
  zoom: 1, panX: 0, panY: 0, drag: null, hovered: null, sequence: 0,
};
const canvas = $("graph");
const ctx = canvas.getContext("2d");
const labels = { pages: "Wiki pages", concepts: "Knowledge concepts", decisions: "Decisions & constraints" };
const graphTitles = { pages: "Wiki relationships", concepts: "Concept connections", decisions: "Decision history & evidence" };
const titleOrder = new Intl.Collator(undefined, { sensitivity: "accent" });
let noticeTimer = null;

function notice(message) {
  const panel = $("notice");
  panel.textContent = String(message);
  panel.hidden = false;
  if (noticeTimer) clearTimeout(noticeTimer);
  noticeTimer = setTimeout(() => { panel.hidden = true; }, 7000);
}
async function request(url) {
  const response = await fetch(url, { credentials: "omit", cache: "no-store" });
  if (!response.ok) {
    let message = "Request failed (" + response.status + ")";
    try { message = (await response.json()).error || message; } catch (_) { /* keep status */ }
    throw new Error(message);
  }
  return response.json();
}
function node(tag, cls, value) {
  const el = document.createElement(tag);
  if (cls) el.className = cls;
  if (value !== undefined) el.textContent = String(value);
  return el;
}
function reset(element) { element.replaceChildren(); }
function kindColor(item) {
  const kind = (item.kind || "").toLowerCase();
  const lifecycle = (item.lifecycle || "").toLowerCase();
  if (["superseded", "proposed", "rejected", "historical", "withdrawn"].includes(lifecycle)
      || kind === "proposal") return "violet";
  if (kind === "constraint" || kind === "risk") return "teal";
  if (kind === "decision") return "amber";
  return "blue";
}
function color(item) {
  const colors = {
    blue: getComputedStyle(document.documentElement).getPropertyValue("--blue").trim(),
    violet: getComputedStyle(document.documentElement).getPropertyValue("--violet").trim(),
    teal: getComputedStyle(document.documentElement).getPropertyValue("--teal").trim(),
    amber: getComputedStyle(document.documentElement).getPropertyValue("--amber").trim(),
  };
  return colors[kindColor(item)] || "#69aaff";
}
function current() {
  if (!state.site) return [];
  return state.site[state.mode] || [];
}
function connections() {
  if (!state.site) return [];
  return state.site[{pages:"page_edges",concepts:"concept_edges",decisions:"decision_edges"}[state.mode]] || [];
}
function matches(item) {
  const q = state.search.toLocaleLowerCase().trim();
  if (!q) return true;
  return [item.title, item.summary, item.kind, item.lifecycle, item.id]
    .some(value => String(value || "").toLocaleLowerCase().includes(q));
}
function renderList() {
  const items = $("items");
  reset(items);
  const found = current().filter(matches);
  found.sort((a, b) => titleOrder.compare(a.title, b.title) || a.id.localeCompare(b.id));
  $("list-title").textContent = labels[state.mode];
  $("list-count").textContent = found.length + (found.length !== current().length ? "/" + current().length : "");
  for (const item of found) {
    const button = node("button", "item" + (item.id === state.selected ? " active" : ""));
    button.type = "button";
    button.dataset.id = item.id;
    const symbol = node("span", "item-icon", state.mode === "pages" ? "▤" : state.mode === "concepts" ? "◈" : "◇");
    symbol.style.color = color(item);
    button.append(symbol);
    const body = node("span");
    body.append(node("span", "name", item.title));
    body.append(node("span", "detail", item.lifecycle || item.kind || item.id));
    button.append(body);
    button.addEventListener("click", () => select(item.id));
    items.append(button);
  }
  if (!found.length) items.append(node("p", "item-note", "Nothing matches. Try a different search."));
}
function keyFor(id) {
  let h = 2166136261;
  for (const c of id) h = Math.imul(h ^ c.charCodeAt(0), 16777619);
  return h >>> 0;
}
function layout() {
  const chosen = current().slice(0, 200); // bounded graphical sample; full list remains accessible.
  const ids = new Set(chosen.map(item => item.id));
  const edges = connections().filter(edge => ids.has(edge.source) && ids.has(edge.target));
  const points = chosen.map((item, i) => {
    const angle = i * 2.3999632 + (keyFor(item.id) % 91) / 230;
    const radius = i ? 58 + Math.sqrt(i / chosen.length) * 155 : 0;
    return { ...item, x: Math.cos(angle) * radius, y: Math.sin(angle) * radius };
  });
  const map = new Map(points.map(p => [p.id, p]));
  // A small offline layout step makes dense reference graphs easier to read.
  for (let turn = 0; turn < 45 && points.length <= 125; turn++) {
    for (let i = 0; i < points.length; i++) {
      const a = points[i];
      let fx = -a.x * .004, fy = -a.y * .004;
      for (let j = i + 1; j < points.length; j++) {
        const b = points[j], dx = a.x - b.x, dy = a.y - b.y;
        const d2 = Math.max(60, dx * dx + dy * dy);
        const f = 310 / d2;
        fx += dx * f; fy += dy * f;
        b.x -= dx * f * .14; b.y -= dy * f * .14;
      }
      a.x += fx * .14; a.y += fy * .14;
    }
    for (const edge of edges) {
      const a = map.get(edge.source), b = map.get(edge.target);
      const dx = b.x - a.x, dy = b.y - a.y;
      const len = Math.max(1, Math.hypot(dx, dy));
      const spring = (len - 95) * .012;
      a.x += (dx / len) * spring; a.y += (dy / len) * spring;
      b.x -= (dx / len) * spring; b.y -= (dy / len) * spring;
    }
  }
  state.points = points;
  state.edges = edges;
  state.panX = 0; state.panY = 0;
  state.zoom = Math.min(1.25, Math.max(.46, Math.min(canvas.clientWidth, canvas.clientHeight) / 510));
  $("graph-count").textContent = chosen.length + " nodes · " + edges.length + " links";
  $("graph-title").textContent = graphTitles[state.mode];
  $("graph-eyebrow").textContent = state.mode === "pages" ? "LINKED DOCUMENTATION" :
    state.mode === "concepts" ? "KNOWLEDGE ZOOM" : "EVIDENCE RELATIONSHIPS";
  paint();
}
function sizes() {
  const ratio = Math.min(window.devicePixelRatio || 1, 2);
  const rect = canvas.getBoundingClientRect();
  const w = Math.max(1, Math.round(rect.width * ratio));
  const h = Math.max(1, Math.round(rect.height * ratio));
  if (canvas.width !== w || canvas.height !== h) { canvas.width = w; canvas.height = h; }
  ctx.setTransform(ratio, 0, 0, ratio, 0, 0);
  return {w: rect.width, h: rect.height};
}
function paint() {
  const {w,h} = sizes();
  ctx.clearRect(0, 0, w, h);
  ctx.save();
  ctx.translate(w / 2 + state.panX, h / 2 + state.panY);
  ctx.scale(state.zoom, state.zoom);
  const byId = new Map(state.points.map(p => [p.id, p]));
  for (const edge of state.edges) {
    const a = byId.get(edge.source), b = byId.get(edge.target);
    if (!a || !b) continue;
    const selected = state.selected && (state.selected === a.id || state.selected === b.id);
    ctx.beginPath(); ctx.moveTo(a.x, a.y); ctx.lineTo(b.x, b.y);
    ctx.strokeStyle = selected ? "rgba(105,170,255,.67)" : "rgba(106,134,171,.24)";
    ctx.lineWidth = selected ? 1.7 : .9;
    if (edge.active === false) ctx.setLineDash([3,4]);
    ctx.stroke(); ctx.setLineDash([]);
  }
  for (const item of state.points) {
    const active = item.id === state.selected;
    const match = matches(item);
    const base = color(item);
    const r = active ? 8.6 : (item.id === state.hovered ? 7.6 : 5.1);
    ctx.globalAlpha = match ? 1 : .14;
    if (active) { ctx.beginPath(); ctx.arc(item.x,item.y,18,0,Math.PI*2); ctx.fillStyle = "rgba(93,156,243,.12)";ctx.fill(); }
    ctx.beginPath();ctx.arc(item.x,item.y,r+3,0,Math.PI*2);
    ctx.fillStyle = getComputedStyle(document.documentElement).getPropertyValue("--bg").trim();ctx.fill();
    ctx.beginPath();ctx.arc(item.x,item.y,r,0,Math.PI*2);ctx.fillStyle=base;ctx.fill();
    ctx.strokeStyle = active ? "#f5fbff" : "rgba(255,255,255,.3)";
    ctx.lineWidth = active ? 1.8 : 0.8;ctx.stroke();
  }
  ctx.globalAlpha=1;ctx.restore();
  const occupied = [];
  const points = state.points.map(item => ({...item, screenX:w/2+state.panX+item.x*state.zoom, screenY:h/2+state.panY+item.y*state.zoom}));
  points.sort((a,b)=>Number(b.id===state.selected||b.id===state.hovered)-Number(a.id===state.selected||a.id===state.hovered));
  ctx.font="500 11px system-ui, sans-serif";
  ctx.textAlign="left";ctx.textBaseline="top";
  ctx.fillStyle=getComputedStyle(document.documentElement).getPropertyValue("--text").trim();
  for(const item of points.filter(matches)) {
    let label=item.title;
    const maxWidth=Math.min(190,w-24);
    while(label.length>1 && ctx.measureText(label).width>maxWidth)label=label.slice(0,-2)+"…";
    const width=ctx.measureText(label).width;
    const left=Math.max(8,Math.min(w-width-8,item.screenX-width/2));
    for(const top of [item.screenY+14,item.screenY-27]) {
      const box={left,top,right:left+width,bottom:top+14};
      if(top<8 || box.bottom>h-8 || item.screenX<0 || item.screenX>w)continue;
      if(occupied.some(other=>box.left<other.right+6 && box.right>other.left-6 && box.top<other.bottom+3 && box.bottom>other.top-3))continue;
      if(points.some(point=>point.screenX>box.left-9 && point.screenX<box.right+9 && point.screenY>box.top-9 && point.screenY<box.bottom+9))continue;
      ctx.fillText(label,left,top);occupied.push(box);break;
    }
  }
}
function worldPoint(event) {
  const rect = canvas.getBoundingClientRect();
  return {x:(event.clientX-rect.left-rect.width/2-state.panX)/state.zoom,
    y:(event.clientY-rect.top-rect.height/2-state.panY)/state.zoom};
}
function hit(event) {
  const point=worldPoint(event);
  let nearest=null, distance=Infinity;
  for(const item of state.points) {
    const d=Math.hypot(item.x-point.x,item.y-point.y);
    if(d<distance && d<Math.max(13,11/state.zoom)){nearest=item;distance=d;}
  }
  return nearest;
}
canvas.addEventListener("pointerdown",e=>{
  canvas.setPointerCapture(e.pointerId);
  state.drag={x:e.clientX,y:e.clientY,panX:state.panX,panY:state.panY,moved:false};
});
canvas.addEventListener("pointermove",e=>{
  if(state.drag) {
    if(Math.hypot(e.clientX-state.drag.x,e.clientY-state.drag.y)>4)state.drag.moved=true;
    if(state.drag.moved) {state.panX=state.drag.panX+e.clientX-state.drag.x;state.panY=state.drag.panY+e.clientY-state.drag.y;paint();}
  } else {
    const found=hit(e)?.id||null;
    if(found!==state.hovered){state.hovered=found;canvas.title=state.points.find(item=>item.id===found)?.title||"";canvas.style.cursor=found?"pointer":"grab";paint();}
  }
});
canvas.addEventListener("pointerup",e=>{
  if(state.drag && !state.drag.moved) { const found=hit(e);if(found)select(found.id); }
  state.drag=null;
});
canvas.addEventListener("pointercancel",()=>{state.drag=null});
canvas.addEventListener("wheel",e=>{
  e.preventDefault();state.zoom=Math.max(.2,Math.min(4,state.zoom*Math.exp(-e.deltaY*.0011)));paint();
},{passive:false});
$("zoom-in").addEventListener("click",()=>{state.zoom=Math.min(4,state.zoom*1.28);paint()});
$("zoom-out").addEventListener("click",()=>{state.zoom=Math.max(.2,state.zoom/1.28);paint()});
$("zoom-reset").addEventListener("click",()=>{state.panX=0;state.panY=0;state.zoom=Math.min(1.25,Math.max(.46,Math.min(canvas.clientWidth,canvas.clientHeight)/510));paint()});
if(window.ResizeObserver)new ResizeObserver(paint).observe($("canvas-wrap"));
else window.addEventListener("resize",paint);

function pill(value) {
  const el=node("span","pill",value);
  const cls=kindColor({kind:value,lifecycle:value});
  if(cls!=="blue")el.classList.add(cls);
  return el;
}
function appendEvidence(parent, records) {
  for (const evidence of records || []) {
    const card=node("div","evidence-card");
    const source=[evidence.root_id,evidence.path].filter(Boolean).join(":");
    const line=evidence.line_start ? " · lines "+evidence.line_start+(evidence.line_end && evidence.line_end !== evidence.line_start ? "–"+evidence.line_end:""):"";
    card.append(node("div","source",source+line));
    card.append(node("p",null,evidence.excerpt || "No stored excerpt in this record."));
    const info=node("div","item-note",(evidence.active?"Current supporting source · ":"Historical supporting source · ")+(evidence.id||""));
    card.append(info);parent.append(card);
  }
}
function addRelations(parent, relations, records) {
  if(!relations || !relations.length)return;
  parent.append(node("div","evidence-label","Documentary relationships"));
  for(const rel of relations){
    const wrap=node("div","linked-record");
    const ref=rel.from===state.selected?rel.to:rel.from;
    const row=node("div","record-head");
    row.append(pill(rel.kind));
    const button=node("button","inline-link",records?.find(record=>record.id===ref)?.title||ref);
    button.addEventListener("click",()=>select(ref));
    row.append(button);wrap.append(row);
    wrap.append(node("p",null,(rel.active?"Current documentary relation. ":"Historical documentary relation. ")+(rel.evidence_id?"Evidence: "+rel.evidence_id:"")));
    parent.append(wrap);
  }
}
async function select(id, updateHistory=true, anchor="") {
  if(state.mode!=="decisions" && !current().some(item=>item.id===id))return;
  const key={pages:"page",concepts:"concept",decisions:"record"}[state.mode];
  const hash="#"+key+"="+encodeURIComponent(id)+(anchor?"&anchor="+encodeURIComponent(anchor):"");
  if(updateHistory && location.hash!==hash)history.pushState(null,"",hash);
  state.selected=id;
  const generation=++state.sequence;
  renderList();paint();
  const item=current().find(x=>x.id===id);
  const body=$("reader-body"); reset(body);
  body.append(node("div","item-note","Loading original knowledge…"));
  $("reader-kind").textContent=state.mode==="pages"?"PUBLISHED WIKI":state.mode==="concepts"?"DERIVED CONCEPT":"DOCUMENTARY KNOWLEDGE";
  $("reader-status").textContent=id;
  try {
    const endpoint=state.mode==="pages"?"page":state.mode==="concepts"?"concept":"record";
    const data=await request("/api/"+endpoint+"?id="+encodeURIComponent(id));
    if(generation!==state.sequence)return;
    const selected=item || {title:data.record.subject,kind:data.record.kind,lifecycle:data.record.lifecycle};
    reset(body);
    body.append(node("h2","doc-title",selected.title));
    const caps=node("div");
    caps.append(pill(selected.kind||state.mode));
    if(selected.lifecycle)caps.append(pill(selected.lifecycle));
    body.append(caps);
    if(state.mode!=="decisions" && selected.summary)body.append(node("p","doc-desc",selected.summary));
    if(state.mode==="pages"){
      const holder=node("article","reader-markdown");
      // Sanitized on the server (raw HTML is escaped; dangerous link/image schemes disabled).
      holder.innerHTML=data.html;
      holder.addEventListener("click",event=>{
        const link=event.target.closest("a[href^='#ku_']");
        if(!link)return;
        const target=document.getElementById(link.hash.slice(1));
        if(!target || !holder.contains(target))return;
        event.preventDefault();
        history.pushState(null,"","#page="+encodeURIComponent(id)+"&anchor="+encodeURIComponent(target.id));
        target.scrollIntoView({block:"start"});
      });
      body.append(holder);
    }else if(state.mode==="concepts"){
      const about=node("p","item-note","A derived navigation grouping, not an accepted policy or independent evidence.");
      body.append(about);
      body.append(node("div","evidence-label","Underlying source records · "+data.total));
      for(const record of data.records || []){
        const row=node("div","linked-record");
        row.append(node("div","record-head",record.subject || record.statement));
        row.append(node("p",null,record.statement));
        row.append(pill(record.lifecycle));
        row.append(pill(record.scope));
        const button=node("button","inline-link","View original support");
        button.addEventListener("click",()=>{
          const area=row.querySelector(".nested-evidence");
          if(area){area.remove();button.textContent="View original support";}
          else {const div=node("div","nested-evidence");appendEvidence(div,record.evidence);row.append(div);button.textContent="Hide support";}
        });
        const actions=node("div");
        actions.append(button);
        row.append(actions);
        body.append(row);
      }
      if(data.omitted)body.append(node("div","warning",data.omitted+" additional records omitted from this bounded reader. Use lore explore or lore evidence for complete retrieval."));
    } else {
      const record=data.record;
      body.append(node("p",null,record.statement));
      body.append(node("div","item-note","Subject: "+record.subject+" · Scope: "+record.scope+" · Support: "+record.support_state));
      body.append(node("div","evidence-label","Original supporting evidence"));
      appendEvidence(body,record.evidence);
      addRelations(body,data.relations,data.related_records);
      body.append(node("p","item-note","A source report is not runtime verification. A proposal does not supersede an accepted decision without supporting authority."));
    }
    body.scrollTop=0;
    if(window.matchMedia("(max-width:790px)").matches && updateHistory)$("reader-body").scrollIntoView({block:"start"});
    if(anchor)document.getElementById(anchor)?.scrollIntoView({block:"start"});
  }catch(error){
    if(generation!==state.sequence)return;
    reset(body);body.append(node("p","error",error.message));
  }
}
function setMode(mode, selectDefault=true){
  if(!labels[mode] || !state.site)return;
  state.mode=mode;state.selected=null;state.sequence++;
  for(const t of document.querySelectorAll(".tab")){
    const chosen=t.dataset.mode===mode;
    t.classList.toggle("active",chosen);
    t.setAttribute("aria-selected",chosen?"true":"false");
  }
  $("search").value="";state.search="";
  renderList();layout();
  const preferred=mode==="pages"?current().find(p=>p.id==="index.md"):null;
  if(selectDefault && (preferred || current().length))select((preferred||current()[0]).id);
  else {reset($("reader-body"));$("reader-body").append(node("p","item-note","No material in this view."));}
}
for(const t of document.querySelectorAll(".tab")) t.addEventListener("click",()=>setMode(t.dataset.mode));
$("search").addEventListener("input",e=>{state.search=e.target.value;renderList();paint()});
window.addEventListener("keydown",e=>{
  if(e.key==="/" && !(e.target instanceof HTMLInputElement)){e.preventDefault();$("search").focus();}
  if(e.key==="Escape" && document.activeElement===$("search")){$("search").value="";state.search="";renderList();paint();}
});
function restoreRoute(){
  if(!state.site)return;
  const params=new URLSearchParams(location.hash.slice(1));
  for(const [key,mode] of [["page","pages"],["concept","concepts"],["record","decisions"]]){
    if(!params.has(key))continue;
    if(state.mode!==mode)setMode(mode,false);
    select(params.get(key),false,params.get("anchor")||"");return;
  }
  setMode("pages",false);select("index.md",false);
}
window.addEventListener("popstate",restoreRoute);
window.addEventListener("hashchange",restoreRoute);
$("theme").addEventListener("click",()=>{document.documentElement.classList.toggle("light");paint()});
$("refresh").addEventListener("click",()=>loadSite(true));
async function loadSite(refresh=false){
  try{
    $("refresh").disabled=true;
    const site=await request("/api/site"+(refresh?"?refresh=1":""));
    state.site=site;
    $("project").textContent=site.project;
    $("stats").textContent=site.pages.length+" pages · "+site.concepts.length+" concepts · "+site.decisions.length+" decision records";
    $("revision").textContent=site.revision ? site.revision.slice(0,18) : "Documentary view";
    $("revision").title=site.revision||"";
    if(site.warnings?.length)notice(site.warnings.join(" · "));
    setMode(state.mode,false);
    restoreRoute();
    if(refresh)notice("Re-read the published wiki and registry. No inference was performed.");
  }catch(error){notice(error.message);if(!state.site){reset($("reader-body"));$("reader-body").append(node("p","error",error.message));}}
  finally{$("refresh").disabled=false;}
}
loadSite();
