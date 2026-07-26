# SPEC-000 Product Scope - POWDERBURN: The Ledger of Elk Creek

Layer L2. Behavior first. Vocabulary defined here is locked and reused by every later spec.

## 1. One paragraph

POWDERBURN is a single-player, offline, deterministic squad tactical role-playing game. The player
commands a company of up to six from a roster of nine across eleven years of the American frontier,
1867 to 1878. Combat is continuous turn based in the Fallout Tactics lineage: an action point
economy, a sequence clock rather than fixed rounds, called shots to seven hit locations, critical
tables, three stances, real line of sight, and squad tactics on an isometric grid. On top of that
lineage POWDERBURN adds the things black powder actually did: smoke that hangs on the field and
blinds both sides, fouling that degrades a cylinder over a long fight, misfires, and reload economies
that make a six-shot cap-and-ball revolver a completely different weapon from an 1873 Winchester. The
story is fictional. The history is not. Real events are immovable weather that the player moves
through and is changed by, never around. Every named person who dies anywhere in the campaign is
written by hand into an in-game document called the Ledger, which is append-only, hash-chained, used
as the save integrity root, and read aloud as the closing credits.

## 2. Problem statement

Tactical RPGs of the Fallout Tactics generation had a combat system with more depth than almost
anything shipped since, and almost nothing has inherited it. Western games, meanwhile, are almost
entirely about a lone gunfighter, and almost entirely treat 1870s America as a backdrop of saloons
and sunsets rather than the specific, documented, catastrophic decade it was. POWDERBURN exists to
put a genuinely deep squad combat kernel inside a Western that takes its own history seriously enough
to name the treaties, the nations, and the dates.

## 3. Target players

Players of Fallout Tactics, Jagged Alliance 2, Silent Storm, X-COM, and Battle Brothers who want AP
depth and permanent consequences. Players of narrative-first RPGs who will stay for a cast of nine
with real arcs. Readers of frontier history who will notice, and care, that the Medicine Lodge
Treaty is on the right date. Modders, who get a documented data format and a validator. All of them
offline: the product never phones home.

## 4. Core user outcomes (ship criteria, each proved by a named live-fire proof)

| ID | Outcome | Proof |
| --- | --- | --- |
| LF-01 | A player starts a new campaign and completes the opening mission Elk Creek end to end, reaching VICTORY, with the in-game Ledger recording every death. | scripts/live-fire.sh LF-01 |
| LF-02 | A called shot to a specific hit location produces the specific mechanical consequence the rules promise: a Broken wound on the gun arm makes the target drop the weapon. | LF-02 |
| LF-03 | The simulation is deterministic. Identical ruleset, content, seed, and input journal produce a byte-identical terminal state hash, three runs in a row, matching the golden corpus. | LF-03 |
| LF-04 | A player saves in the middle of a firefight, loads, and the simulation state is exactly what it was, with the Ledger hash chain intact. | LF-04 |
| LF-05 | A companion who dies stays dead: they are memorialized in the Ledger and every later reference to them is gated, with zero dangling references anywhere in reachable content. | LF-05 |
| LF-06 | A moral choice in Act II really changes what content exists in Act III; two scripted playthroughs from the same company seed diverge into different missions. | LF-06 |
| LF-07 | History cannot be rewritten: content that would alter a HISTORICAL_FIXED outcome is rejected by the validator with error E-HIST-001. | LF-07 |
| LF-08 | The renderer really draws the game: a headless capture on a software adapter produces a non-blank frame whose hash matches the golden frame. | LF-08 |
| LF-09 | Every depicted nation and community carries its specific name and its sources, and every asset carries its license. | LF-09 |
| LF-10 | A sixty-actor battle resolves an AI turn within 120ms and a simulation step within 16ms on the reference machine. | LF-10 |

## 5. Setting and story

### 5.1 Premise

August 1867. Elias Ward was a hospital steward with the Army of the Tennessee, which means he spent
the war holding men down while other men cut. He came home to the Kansas and Missouri border, which
by then was not a border but a scar: Jayhawkers on one side, Quantrill's leavings on the other, and
between them a strip of burned farms nobody was rebuilding. On a dead paymaster at a wash called Elk
Creek he found a bound ledger. It was an accounting: Army quartermaster stock, annuity goods promised
by treaty, and surplus rifles, all moving in a circle between a cartel of financiers, agents, and
officers who had worked out that a frontier at war is a frontier where land is cheap. The men in that
book are called the Elk Creek Ring. Elias cannot read the book without becoming responsible for it.
That is the whole game.

### 5.2 The Ledger (the emotional and mechanical spine)

Elias keeps writing in it. Every named character who dies in the campaign, ally or enemy or bystander,
gets an entry in his handwriting: name, place, date, and one line he chooses from three offered. The
Ledger is append-only and hash-chained; entry N carries the hash of entry N-1, and the chain head is
the integrity root of the save file. It is not decoration. Its length feeds a value called Weight,
which slowly closes off glib dialogue options, slows Sand recovery between missions, and opens a
final act of dialogue available no other way. At the end the game reads the Ledger back, in order,
and that is the credit roll. A player who fought carefully and a player who burned through the West
get different endings not because of a morality meter but because they wrote different books.

### 5.3 The company (nine recruitable, permadeath, each with a real arc)

| ID | Name | Who | Arc anchor |
| --- | --- | --- | --- |
| c_elias | Elias Ward | Player character. Union hospital steward turned marksman. Carries the Ledger. | The question of whether an accounting is justice |
| c_naomi | Naomi Freed | Gunsmith, born enslaved in Tennessee, freed at a contraband camp, working her way west | The founding of Nicodemus, Kansas, in 1877 |
| c_whitehorse | Tsayd-tainte, called Whitehorse | Kiowa scout whose band was broken in the campaigns after Medicine Lodge | The 1875 deportation of seventy-two prisoners to Fort Marion, Florida |
| c_doyle | Sgt. Absalom Doyle | 10th Cavalry sergeant, Buffalo Soldier, twelve years in | Duty against the campaigns he is ordered to fight |
| c_ruelas | Ignacio Ruelas | Tejano vaquero whose family land was taken by the land courts | The El Paso Salt War, 1877 |
| c_wen | Wen Li-hsiang | Central Pacific blast man, laid off at Promontory | The 1871 Los Angeles massacre and the 1875 Page Act |
| c_ames | Cordelia Ames | Pinkerton operative who walks away from the agency | The Great Railroad Strike of 1877 |
| c_mercer | Hollis Mercer | Unreconstructed Confederate cavalryman, still telling himself a story | The story failing |
| c_alcantara | Tomas Alcantara | Defrocked priest, field surgeon, drinker | The yellow fever epidemic of 1878 |

Antagonist: Colonel Ambrose Teague, quartermaster turned railroad land agent, architect of the Ring.
He is not a sadist. He believes the suffering of the frontier is the price of a continent and he can
show you the arithmetic. The last conversation with him is the last real choice in the game.

### 5.4 Structure

Four acts, twenty-four missions, twelve camp interludes. Missions carry a date. Camp interludes are
where the Ledger gets written, wounds heal, Sand recovers, and companions talk.

Act I, Bleeding Ground, 1867 to 1868. Kansas and Indian Territory. The Medicine Lodge Treaty is
signed in October 1867 and the company watches it happen. Act I ends in the winter after the Washita.

Act II, The Iron Road, 1869 to 1873. The rails meet at Promontory Summit on 10 May 1869 and the
company is in the crowd. The Kansas Pacific reaches Denver in 1870. The Panic of 1873 arrives on
18 September and takes the company's patron with it.

Act III, The Hide Trade, 1874 to 1876. Adobe Walls, 27 June 1874. The Red River War. Black Hills
gold. The company is a long way from the Little Bighorn on 25 June 1876 and hears about it four days
later, which is the point.

Act IV, The Reckoning, 1877 to 1878. Federal troops leave the South. The Nez Perce flight runs from
June to October 1877. The Salt War. The Lincoln County War, February to July 1878. Teague, and the
Ledger, at the end.

### 5.5 The historical immutability law (LBI-05)

Every real event is a content node tagged HISTORICAL_FIXED with its date, its outcome, and its
citations. Such a node has no outgoing edge that changes its outcome. The player can be present, can
be hurt by it, can save one specific fictional person inside it, and can never alter it. The
validator enforces this structurally and rejects violations with E-HIST-001. This is a design law,
not a limitation: the game is about what it is like to live inside events you cannot move.

## 6. Non-goals

No multiplayer, no co-op, no online anything. No telemetry, no analytics, no crash upload, no
account, no launcher, no DRM. No real-time-with-pause mode; the sequence clock is the whole point.
No procedural story generation. No native-code mods; data mods only, sandboxed. No microtransactions
and no post-launch content sold separately. No macOS build in v1. No mobile. No scalping, no bounty
mechanic that pays for human remains, and no faction alignment slider that sorts peoples into good
and bad.

## 7. Representation law (LBI-06, a hard gate, not a guideline)

1. Every Native character record carries a `nation` field naming a specific nation (Kiowa, Comanche,
   Southern Cheyenne, Lakota, Chiricahua Apache, Nez Perce), never a generic term, and a `sources`
   array of at least two citations.
2. Every Black, Tejano, Mexican, and Chinese character record carries a `community` field and the
   same `sources` requirement.
3. Dialogue written in Kiowa, Comanche, Spanish, or Cantonese appears in that language with a
   translation line and a `translation_source` field. It is never rendered as broken English.
4. No faction has an alignment value. Factions have interests, recorded as a `interests` array.
5. A forbidden-token list ships in `content/lint/forbidden_tokens.txt`. Any occurrence anywhere in
   `content/` fails `pbtool validate representation`.
6. No mechanic converts a human body or a human remain into currency, loot, or a resource.
7. The bibliography ships with the game as `content/BIBLIOGRAPHY.md` and is reachable from the main
   menu.

These rules exist because a game that names the Medicine Lodge Treaty has taken on an obligation, and
because they are cheap to enforce mechanically and expensive to retrofit.

## 8. Success metrics

Every one of the ten core outcomes proved by live fire on a clean tree. Full campaign replay of the
scripted golden playthrough completes in under nine minutes of wall clock on the reference machine.
Zero network syscalls in the shipped binary. Content validator clean on the whole tree. Reproducible
build: two builds from the same commit produce identical artifact hashes.

## 9. Production readiness

Defined in SPEC-008 and enumerated line by line with a verifying command in PRODUCTION_READINESS.md.
