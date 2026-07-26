NODE-META-BEGIN
ID: EP-XXX
DEPS: -
MAX_ATTEMPTS_PER_MILESTONE: 6
VERIFY: sh scripts/verify.sh
VERIFY_SENTINEL: verify: ok
GREEN_TAG: green/EP-XXX
NODE-META-END

# EP-XXX <title>

## 1. Purpose and Big Picture
One paragraph: why this node exists and what the repository looks like when it is done.

## 2. Scope
Bullets. What is in this node.

## 3. Non-goals
Bullets. What is deliberately not here, naming the node that owns it instead.

## 4. Context and Orientation
The state of the repository at entry, the green tag it starts from, and the invariants in play.

## 5. Files to Read First
Exact paths, in reading order.

## 6. Expected Changed Files
Exact paths. This is the audit list checked at node end.

## 7. Interfaces and Contracts
Types, signatures, content keys, and command surfaces taken verbatim from the specs.

## 8. Milestones

### M1: <name>
GOAL:
READ:
CHANGE:
CONTENT:
RUN:
EXPECT:
EVIDENCE: sh scripts/ledger.sh append <AGENT_ID> EP-XXX MILESTONE_PASS "M1 <sentinel>"
FALLBACK:
COMMIT: git add -A && git commit -m "[EP-XXX][M1] <summary>"

## 9. Validation and Acceptance
Each criterion with the command and sentinel that proves it.

## 10. Idempotence and Recovery
Exact reset command and what state it restores.

## 11. Progress
- [ ] M1

## 12. Surprises and Discoveries

## 13. Decision Log

## 14. Outcomes and Retrospective
