# Runbook: <situation>

## Symptom
What the operator or the player actually sees, in their words.

## Impact
Who is affected and how badly. If the answer is nobody, say so and stop.

## First diagnostic
One command. The single fastest thing that distinguishes the likely causes.

## Decision tree
| Observation | Cause | Go to |
| --- | --- | --- |

## Mitigation
Exact commands, in order, with what each one should print.

## Verification
The command that proves the situation is over.

## Rollback trigger
The condition under which this stops being a fix and becomes a ROLLBACK.md trigger.

## Follow up
The gate, test, or matrix row that must exist so this cannot recur silently.
