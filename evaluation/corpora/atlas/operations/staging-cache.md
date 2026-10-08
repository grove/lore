# Staging cache implementation

The staging authentication service uses an in-process cache so test runs remain isolated. The production authentication service uses Redis under ADR-009. These are different environment-specific configurations, not conflicting statements about the same deployment.
