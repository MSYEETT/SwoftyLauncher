# F1 1990

Turns the F1 2026 season into the 1990 season in a profile of its own.

## Rules (db/rules.json, db/rules_text.json, tools/rules.py)

`tools/rules.py` writes both op files. In the `f1_26` setup mode:

- Weekend (default structure): P1, P2, Friday qualifying (a single sprint-shootout session), Saturday qualifying (a single qualifying session), race. Full qualifying runs the single sessions; the race grid comes from Saturday qualifying. Every 2026 track runs the default format, and Grand Prix only offers the standard weekend.
- Tyres: every allocation preset hands out `soft_sets` softer-compound sets in P1, P2, Friday qualifying, Saturday qualifying and the race, and no medium or hard sets. The softer compound is C4 (`sup`) on every 1990 calendar track and shows as Supersoft (`SuperSoft` sidewall context). Mandatory-tyre presets are empty, the calendar tracks require 0 pit stops, and there are no tyre warmers.
- Race control: safety car off and parc fermé off, as defaults and as the only values the `gamesetup_f1` tag allows. Red flags stay. DRS uses the `f1_1990` lap group, which opens on lap 200.
