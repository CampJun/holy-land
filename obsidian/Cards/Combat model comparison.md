Pros/cons of three roguelike combat philosophies, in the context of Holy Land's gamepad-first, blessing-driven, oasis→wilderness loop. Decide direction before deepening the combat draft (`[[Action point system]]`, `[[Combat math foundation]]`, `[[Power system]]`, etc.).

## TL;DR

- **Current (AP-per-turn)** — tactical-RPG identity. Each actor gets a 100-AP turn budget; weapons cost 25–100 AP per swing. Reads like Battle Brothers / FF Tactics in a roguelike shell. Simple HUD, clean tradeoffs, but speed differences are coarse.
- **ToME 4 (energy/speed)** — action-economy identity. Actors accumulate energy each tick at a `speed` rate; act when energy ≥ 1000; haste/slow are first-class. Faster characters can take 2–3 actions per slow enemy turn. Strong tempo expression, more opaque HUD.
- **DCSS (auts)** — continuous-tempo identity. Every action has an "aut" cost; the engine picks whoever has the lowest aut counter next. Weapon skill *reduces delay* (more swings per real-time, not per turn). Granular speed expression, but the "turn" concept blurs.

Holy Land's current pick is **AP**, and several other draft cards (skill reducing swing AP cost, attribute → Speed reducing move cost, AP-priced item use) already lean into it — though "Attack skill reduces swing AP cost" is actually a DCSS-shaped mechanic in AP clothing.

---

## Model 1 — Current AP (100 AP/turn, non-accumulating)

**Mechanics recap** (from `[[Action point system]]`):
- Each actor gets 100 AP per turn; unused AP does NOT carry over.
- Move = 25 AP (Speed reduces it). Careful step = 50 AP (avoids opportunity attacks).
- Weapon swings = ~25 (dagger) / ~50 (1H sword) / ~80 (2H). Net DPS roughly comparable; texture differs.
- Powers = 25 / 50 / 75 AP by tier, also paying mana + cooldown.
- Items = 50 AP base, 25 with 1H+empty-offhand loadout.
- Grunts get a flat "AP per turn" stat (typically 100). Bosses get full attributes.

**Pros**
1. **Legible HUD.** A single 100-AP bar on a 640×480 gamepad UI is trivial to render and read at a glance. No "next-actor" indicator needed.
2. **Tactical decision density.** "I have 100 AP, what's the best combination?" is the FF Tactics / Battle Brothers puzzle — strong fit for a turn-based, gamepad-paced experience.
3. **Clean weapon texture.** Dagger = 4 small hits + 1 power, 2H = 1 big hit + careful step + power. Different rhythms without different math.
4. **Asymmetric enemies are cheap.** Grunts get hand-tuned numbers ("100 AP/turn, 25/move, 50/swing"); no need for a full speed/attribute schema. `[[Enemy tiered stat blocks]]` already commits to this.
5. **Powers stay special.** A 75-AP heavy power blows most of your turn — it FEELS expensive without separate accounting.
6. **No interleaving surprises.** You finish your turn, then enemies act, then you act. Predictable; matches the contemplative pace of save-anywhere portable play.
7. **Blessings stack cleanly.** `[[Blessing system]]` and `[[Blessing combo and stacking]]` modify per-power numbers; no global time line to perturb.

**Cons**
1. **Coarse speed.** Hasted/slowed creatures need ad-hoc rules ("gets 150 AP this turn?" "moves at 15 AP/square?"). No first-class way to express "this monster is twice as fast."
2. **End-of-turn dead AP.** If you have 24 AP left and every option costs 25+, you waste it. Encourages floor-sweeping optimization that becomes tedious.
3. **Crowd combat drags.** Round-robin turns × N enemies × 100 AP each = wait time grows linearly with mob size. Roguelike density (10+ enemies on screen) exposes this fast.
4. **Status effects are ad-hoc.** Haste = "reduce move cost 50%" or "+50 AP this turn"? Both are hacks vs. multiplying a speed coefficient.
5. **Initiative is unspecified.** Currently silent on who acts first inside a turn — if it ends up being Speed-based, that's reinventing a small energy system in worse clothes.
6. **Free actions are awkward.** Looking, pickup, equip-swap need either arbitrary 0-AP exemptions or to feel like "real" actions costing 25–50 AP each.
7. **"Turn" boundary feels artificial in a roguelike.** Most genre veterans expect continuous tempo; the cap may read as "JRPG with ASCII" rather than "true roguelike."

---

## Model 2 — ToME 4 energy/speed

**Mechanics recap.** Every game tick, each actor gains `speed` energy (baseline speed = 1.0 → +1000/sec of normalized time). When energy ≥ 1000, the actor can act; actions consume 1000 energy. Haste 1.5 → 1500 energy/tick → 1.5× as many actions over time. Cooldowns are tracked in "game turns" (normalized real-time units), decoupled from the actor's own turn count. The "current actor" is whoever's energy first reaches the act threshold.

**Pros**
1. **Speed is first-class.** Haste, slow, swift, and quickness are single-value modifiers. No special-case AP arithmetic — just multiply.
2. **Tempo combos emerge.** "Cast haste, then kill 4 imps before they react" is a natural buildup, not an exception.
3. **Cooldown intuition.** "Cooldown = 4 turns" means real-time turns; a hasted player gets back to their cooldown faster *automatically* — feels rewarding.
4. **Weapon speeds compose.** Daggers can have lower energy cost per swing → more swings per second of game-time, not per artificial turn cap.
5. **Familiar to ToME/Angband audience.** A meaningful subset of roguelike players already grok this.
6. **Boss design lever.** A boss with speed 0.7 attacks less often but hits harder — a clear and balanced asymmetry.

**Cons**
1. **HUD complexity.** A 640×480 screen with no mouse needs an explicit "next-actor" indicator or energy bars on every visible entity. Doable but eats real estate.
2. **Player loses the "full turn."** You'll often act, then an enemy acts twice, then you again. The tactical-puzzle feel of "spend my 100 AP" disappears.
3. **Powers blend in.** A power costs 1000 energy like everything else; without explicit cost framing, deity abilities feel like just another attack.
4. **Grunt schema bloats.** Every grunt needs a speed stat *and* per-action energy costs; the "just tune the numbers" cheapness from `[[Enemy tiered stat blocks]]` partially evaporates.
5. **Blessing stacking gets thorny.** `[[Blessing combo and stacking]]` is currently multiplicative on power damage; mixing in haste-stacking (which multiplies actions, which multiplies *output*) creates a second hidden multiplier that can spiral.
6. **Cooldown ambiguity.** Real-time turns vs. personal-turn cooldowns is a small explainer paragraph in tutorials. Easy to misread.
7. **Opaque "why did this happen."** "Why did the enemy go twice?" is a common new-player question and requires UI affordances to explain.

---

## Model 3 — DCSS auts

**Mechanics recap.** Each action has an "auts" cost. Reference unit: 10 auts = 1 "turn." Movement is 10 auts at normal speed. Weapon delay = `base_delay × max(min_factor, 1 − skill/20)` — i.e. training a weapon makes it swing faster, not (only) hit harder. Player and monsters all carry a personal aut counter; engine picks lowest counter next. Free actions (look, swap, examine) cost 0 auts. Monster speeds are auts-per-action (7 = fast bat, 14 = sluggish drake).

**Pros**
1. **Skill investment is viscerally felt.** Each skill point in a weapon line shortens delay. After hours of training, your dagger swings noticeably faster — progression you SEE in tempo, not just bigger numbers.
2. **Per-action speed.** A creature can be fast in melee but slow at casting (or vice versa) by giving spells and swings different aut costs. Cleaner expressiveness than a single Speed stat.
3. **Free actions are well-defined.** Inventory ops cost 0 auts — no awkward "is sorting my pack a tactical decision?" question.
4. **Granular weapon variety.** Daggers (~5 auts trained) vs. great maces (~20 auts) vs. whip (long reach, medium delay) all coexist; the "all weapons equal DPS" texture of AP doesn't have to hold.
5. **Roguelike-native pacing.** DCSS is the genre default; most players intuit the model without explanation.
6. **Hybrid-friendly.** You can surface auts to the player as "you swing X% faster" or "next action in ~3 auts" without forcing the math on them.

**Cons**
1. **"Turn" is ambiguous.** Cooldowns in auts? In normalized turns? In personal turns? Pick one and explain.
2. **Skill specialization tax.** Reducing weapon delay incentivizes investing heavily in one weapon line — DCSS itself struggles with this (players hoard skill XP). Less interesting if you want broad weapon variety in a single run.
3. **HUD math gets fiddly.** Showing "your attack delay: 7.2 auts" is more numerical than "swing costs 50 AP." Hard to make beautiful on a small gamepad screen.
4. **Powers cost what?** Deity powers need their own aut cost story. Easy to make them all 10 auts, but then they don't *feel* special. Or you give them long auts AND mana AND cooldowns, which is three accounting systems.
5. **Bosses need more knobs.** Movement-aut, attack-aut, and per-spell-aut all distinct. Tuning surface grows.
6. **Loses the "spend my budget" puzzle.** No "I have 100 AP, what's the best combo?" decision frame — each action is just "is now the time?"
7. **Blessing model misfit.** `[[Blessing combo and stacking]]` is built around per-power multipliers. Mapping that onto a delay-based system needs translation (does a blessing reduce auts? add free auts? buff damage despite delay?).

---

## Cross-cutting concerns specific to Holy Land

1. **Gamepad-first 640×480 HUD.** The simpler the model, the better the on-screen affordance budget. AP wins on legibility; energy/aut systems need extra UI (next-actor, per-entity bars) that competes with the AP bar, target info, and power slots already planned in `[[Combat HUD]]`.

2. **Boss/grunt asymmetry.** `[[Enemy tiered stat blocks]]` already commits to "grunts get cheap numerical stats, bosses get the full schema." AP supports this best (one stat: "AP per turn"). Energy/auts pull bosses and grunts toward a shared model.

3. **Deity power texture.** The blessing+power system is the game's emotional core (the "Holy Land" identity). Powers want to feel *distinct* from basic attacks. AP can give them their own slot framing on the HUD ("hold modifier to reveal"). Energy/auts treat them as just another action with cost — easier to make them feel transactional rather than divine.

4. **Blessing stacking compatibility.** `[[Blessing combo and stacking]]` is multiplicative on per-power numbers. AP plays nicest because powers have a fixed per-turn slot to be modified. Energy systems risk a second hidden multiplier (haste × damage = doubled output).

5. **"Stardew day → wilderness night" rhythm.** The macro loop is contemplative (sim daytime) punctuated by deadly (night raids). Tactical, full-turn combat with discrete decisions matches this better than continuous-tempo high-density encounters.

6. **Skill→cost already DCSS-shaped.** `[[Skill system]]` says "Attack skill reduces swing AP cost" and "Magic skill reduces power AP cost." That's literally DCSS's weapon-skill-reduces-delay, ported into AP units. So a *partial* DCSS borrow already exists in the design and works fine.

7. **Save schema cost.** Saves carry attribute/skill state. Switching to energy/auts adds an `energy: i32` and `next_act_at: i64` per actor — a v1→v2 schema bump that has to coexist with `[[Save schema migration testing]]`. AP's "everyone resets to 100" needs zero per-actor state.

8. **Miyoo CPU budget.** Energy/aut systems re-sort the actor queue every action. With a few dozen actors on screen, fine. With chunked wilderness (`[[Chunk-based world persistence]]`), the offscreen actor count matters — AP's strict per-turn loop is easier to bound.

---

## Hybrid options

These are blends that aim to keep most of one model's identity while borrowing tempo or simplicity from another.

### Hybrid A — AP with Speed-based initiative (smallest change from current)
- Keep 100 AP/turn for everyone, but order actors within a turn by a Speed score (Agility-derived).
- Fast actors act first within the round; slow actors last.
- "Haste" = +N initiative; "Slow" = pushes you to act after some enemies that round.
- **Wins**: keeps the HUD, keeps the puzzle, fixes the "initiative unspecified" gap.
- **Loses**: no extra-actions tempo (a 2× speed monster still gets one turn, just earlier).

### Hybrid B — DCSS auts, surfaced as a "100 AP bar"
- Internally use auts. Display as a bar that drains; show "next monster acts in X" beside it.
- Weapon skill reduces auts (i.e. the bar drains slower per swing) — this *already* matches the planned skill→cost coupling.
- Powers get a chunky aut cost (matching today's 25/50/75 framing) plus mana plus cooldown.
- **Wins**: legible HUD + per-action speed expressiveness + skill investment feels real.
- **Loses**: internal complexity (next-actor scheduling), bosses need fuller stat lines.

### Hybrid C — ToME energy, with grunt asymmetry preserved
- Player and bosses use real ToME-style speed/energy.
- Grunts get a simple "acts every N player-actions" rule (no energy bookkeeping).
- **Wins**: keeps grunt tuning cheap; gets haste/slow as first-class for the player.
- **Loses**: two combat models running in parallel; harder to playtest; the "in-striking-range" alert (`[[Combat HUD]]`) has to handle both.

---

## Open questions for you

1. Is the **tactical-RPG full-turn feel** (Battle Brothers / FF Tactics in a roguelike skin) core to Holy Land's identity, or negotiable? — This is the single biggest fork.
2. How important is **first-class haste/slow** to the blessing design? If a god of speed is meant to grant 2× action rate, AP makes that awkward; ToME makes it trivial.
3. How important is **weapon variety**? If you want 10+ weapons with distinct tempo signatures, DCSS-style per-action delay is the easiest to balance. If 4–6 weapons with texture-not-tempo is enough, AP suffices.
4. Should **deity powers be expensive-feeling**? If yes, AP's slot-and-cost framing is the easiest win; energy/auts require explicit UI work to keep them special.
5. Are you willing to spend HUD real estate on a **next-actor indicator** and per-enemy energy bars? If no, AP or Hybrid A is the only safe path on the Miyoo's screen.
6. Does the planned `[[Save schema migration testing]]` deserve to be exercised by a real combat-model bump (going v1→v2 carries actor energy state), or should the v1→v2 bump come from `[[Chunk-based world persistence]]` instead?

---

## Recommendation framing (not a decision)

If the answers above lean "tactical, gamepad, blessings-distinct, fewer-weapons-with-texture" → **keep AP, adopt Hybrid A** (Speed-based initiative) for the missing piece, and absorb DCSS's "skill reduces delay" idea via the already-planned AP-cost-reduction-on-skill mechanic.

If the answers lean "fast tempo, expressive speed, more weapons, fine with HUD complexity" → **adopt Hybrid B** (auts internally, AP bar externally). It's the more ambitious build but preserves the player-facing HUD you've already designed.

Pure ToME-energy is probably the worst fit: it loses the most legibility AND complicates the blessing/power identity the game is built around.

Source: drafted 2026-05-17 in response to "let's rethink the combat system."
