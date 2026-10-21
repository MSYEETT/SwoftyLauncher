# Economy

Owner: `native/src/economy.cpp` and `native/src/components.{hpp,cpp}` (ce-econ). Ops run on the flow
thread while `ce::safe()` (plain `CE_OP`). Every op reads the value through the owner, applies the change
through the game's own path, reads it back and returns `before`/`after`. Every op logs one line:
`[career-editor] economy.<op> <who> <before> -> <after> (<path>)`.

Errors (`ok: false`, `error`): the string starts with the code, optionally followed by `: <detail>`.
Codes: `not_ready` (no championship entity, or the owning component is missing),
`wrong_mode` (the op is My Team only and the career is Driver Career), `bad_args`, `no_player` (the
player's entrant was not found, or a two-player career has more than one human entrant), `no_account`
(no bank account for the player's team), `expect_mismatch` (the live value differs from `expect`),
`locked` (the RP wallet is flagged unlimited by the game), `game_refused` (the game's path ran but the
value did not move as asked; `after` holds the live value).

`expect` (optional, every op): the value the launcher showed. The op is rejected with
`expect_mismatch` when the live value differs.

## Cash (My Team only)

The team's money is a `CareerBankComponent` account: a vector of sub-accounts (category 1 = team cash),
each an int32 balance plus a 0x14-byte transaction history. The edit posts the game's own
`BankAccountTransactionMessage` on the game message bus. The bank's own handler applies it on the
current day: it adds the amount (saturating at 2,147,483,647), appends a history record (so the
Finance History graph shows it), splits income into the budget pools like any income, and posts the
game's "transaction applied" message so the Team HQ UI refreshes. The transaction is posted without the
affordability check, with category 1 (team cash) and day mode "immediate".

Transaction type (`type` arg): `"reward"` (default) = `eMyTeamReward` (27), which the bank never scales
by the owner-perk income multiplier, so the amount lands exactly; `"debug"` = the game's own `eDebug` (8)
type (income of this type is scaled by the perk multiplier on teams that have one, and its Finance
History label is untested).

### `economy.cash.set`
Args: `{ "value": int 0..2147483647, "expect"?: int, "type"?: "reward"|"debug" }`. Posts one
transaction for `value - live`. No-op (ok, `before == after`) when equal.

### `economy.cash.add`
Args: `{ "amount": int -2147483648..2147483647 (≠ 0), "expect"?: int, "type"?: "reward"|"debug" }`.
The result is clamped to 0..2147483647 (a negative amount larger than the balance only takes the
balance to 0).

Result (both): `before`/`after` = `{ "cash": int, "team": u32 }` (`team` = the game's packed team key,
team stat id in the high 16 bits). Undo: the inverse transaction (`economy.cash.set` to `before.cash`),
which adds a second history record; the graph keeps both.

## Resource points (both modes)

RP are a spendable wallet in `ResourcePointsComponent`, kept either per team (wallet keyed by the team
key, the My Team layout) or per driver (keyed by the driver key); the component's own mode byte says
which. The edit calls the wallet adjust function the game's own award handlers call
(`MessageResourcePointsAwarded`, `resource_points_generation_message` and
`MessagePassiveObjectiveAwarded` all end in it): it adds the delta, clamps at 0, and posts the game's
`resource_points_adjusted_message` so the RP counters refresh (per-driver mode: not for a reserve seat,
entrant `+0x80 >= 2`; the op's read-back goes through the wallet getter, never through that message). The award multipliers (contract perk,
difficulty setting) are not applied, so the wallet moves by exactly the amount asked. Only the
player's wallet is touched (the player's entrant is the one human entrant; AI wallets are never
written).

Range: the wallet is kept in 0..999,999 (the player card shows 1,000,000 as "unlimited").

### `economy.rp.set`
Args: `{ "value": int 0..999999, "expect"?: int }`. Applies `value - live` through the adjust path.

### `economy.rp.add`
Args: `{ "amount": int -999999..999999 (≠ 0), "expect"?: int }`. The result is clamped to 0..999,999.

Result (both): `before`/`after` = `{ "rp": int, "key": u32, "per": "team"|"driver" }`.
Undo: `economy.rp.set` to `before.rp`.

## Renown (My Team)

Renown lives in `MyTeamRFComponent` as one object per team (renown points as a float, tier
thresholds from the gamedb). The edit posts the game's own `MessageRAwarded` for the player's team,
the message the `MyTeamRenownReward` grant posts ("Gives %.2f renown to the team immediately"). Each
team's renown object handles it: it adds the amount, posts the tier-up message when a threshold is
crossed, and grants or revokes the renown-tier rewards for the new level.

### `economy.renown.set`
Args: `{ "value": number 0..100000, "expect"?: number }`. Posts one award of `value - live`.

### `economy.renown.add`
Args: `{ "amount": number -100000..100000 (≠ 0), "expect"?: number }`. The result is clamped to 0..100000.

Result (both): `before`/`after` = `{ "renown": number, "level": int, "team": u32 }` (`level` = how many tier
thresholds the points reach, as the player card counts it). Undo: `economy.renown.set` to
`before.renown`. Tier rewards that the game granted on the way up are revoked by its own re-evaluation
when the points drop back.

## Sponsors (My Team)

Read: every sponsor of the player's team (id, name, active, slot, loyalty, loyalty level, top level XP).
Edit: loyalty XP for the active primary sponsor, through the game's own
`PrimarySponsorLoyaltyXPGainedMessage` (the message the primary-sponsor loyalty reward posts).
`CareerSponsorsComponent` handles it: it applies the team's loyalty multipliers for that reason,
clamps at the top loyalty level, and claims every loyalty reward the new XP reaches. Contract swaps and
objective payouts are not offered: their game paths go through the sponsor-negotiation screens.

### `economy.sponsor.loyalty.add`
Args: `{ "amount": int 1..100000, "reason"?: int 0..5 = 0, "expect"?: int }`. Reasons (the game's
enum): `0` CompletedRaceWeekendPrimary, `1` CompletedRaceWeekendSecondary, `2` SeasonalGoal,
`3` FacilityRequest, `4` DepartmentEventOutcome, `5` ActivityOutcome. The game scales the amount by
the team's perks for that reason, so `after.loyalty - before.loyalty` can differ from `amount`.

Result: `before`/`after` = `{ "team": u32, "sponsor": int, "name": string, "loyalty": int, "level": int }`.
Undo: backup-only. The game has no loyalty-removal path, and claimed loyalty rewards stay claimed.

## Cost cap (My Team only)

`CostCapComponent` keeps, per team and per season, the spending counted against the cap
(`cached_expenses`), the cap itself (`cost_cap`) and a modifier (`cached_modifier`). The game moves the
spending only from bank transactions: checked facility running costs, engineering facility upgrades
and upgrade development add to it, and a damage refund takes `amount × modifier` off it. The cap value
is computed by the game, so there is no edit op for the cap itself.

### `economy.costcap.refund`
Args: `{ "amount": int 1..100000000, "expect"?: int }` (`expect` = the current season's expenses).
Posts a `BankAccountTransactionMessage` of type `eDamageRefund` (6), unchecked, team cash, today, which
is exactly what the game's damage refund does. Two effects: cash goes up by `amount` (with a Finance
History record, no perk multiplier) and this season's cost cap expenses go down by `amount × modifier`.
Rejected when it would take the expenses below 0 (the error names the largest allowed amount).

Result: `before`/`after` = `{ "season": int, "expenses": int, "cost_cap": int, "cash": int }`.
Undo: backup-only (cash can be taken back with `economy.cash.add`, but the expenses can't be raised
again).

## Snapshot section `economy`

```json
{
  "available": true,
  "mode": "my_team" | "driver_career" | "none",
  "player": { "driver": u32, "team": u32 } | null,
  "cash": null | {
    "balance": int,
    "team": u32,
    "accounts": [ { "category": int, "balance": int, "min": int } ],
    "history": [ { "type": int, "type_name": "eMyTeamReward", "amount": int, "day": int } ]
  },
  "rp": null | { "wallet": int, "per": "team"|"driver", "key": u32, "unlimited": bool },
  "cost_cap": null | {
    "current_season": int,
    "seasons": [ { "season": int, "expenses": int, "cost_cap": int, "modifier": number } ]
  },
  "sponsors": null | {
    "primary": int,
    "list": [ { "id": int, "name": string, "active": bool, "slot": int, "loyalty": int, "level": int,
                "max_loyalty": int } ]
  },
  "renown": null | { "points": number, "level": int, "tiers": [ int ] }
}
```
`cash`, `cost_cap`, `sponsors` and `renown` are null outside My Team. `history` holds the last 20
records of the team cash account, newest last; `day` is the career day (the game keeps minutes,
`/1440`). `sponsors.primary` is the sponsor that earns loyalty (`-1` when none). Sponsor `name` is the
game's internal name string (it may be empty when the game keeps none); `slot`/`active` are raw flags.

## Bindings

All game addresses and layouts are KB rows (`re/db`), exported by `bindgen` into `swofty::game`.

| kind | names |
|---|---|
| core library | `swofty::career::entity`, `component_by_desc`, `post` (the game's `Ne_Post_BusMessage` on the live bus); `swofty::mem::readable`, `read`, `in_image`, `object_like`, `c_string` |
| functions | `Career_Get_RpWallet`, `Career_Adjust_RpDriverWallet`, `Career_Adjust_RpTeamWallet`, `Career_Get_TeamRenownData` |
| globals | the `k_Career_<Class>ComponentDesc` descriptors, `k_Career_ChampionshipDesc` |
| vtables | `vt_Career_BankTransactionMessage`, `vt_Career_RenownAwardedMessage`, `vt_Career_SponsorLoyaltyXpMessage`, `vt_Career_Championship` (season lane) |
| structs (core) | `NeSharedCtrl` |
| structs (career) | `CareerEntity`, `CareerTimeComponent`, `CareerBankComponent`, `BankAccount`, `BankSubAccount`, `BankTransaction`, `BankTransactionMessage`, `ResourcePointsComponent`, `TeamRenownData`, `RenownConfig`, `RenownAwardedMessage`, `CareerSponsorsComponent`, `SponsorTeamRecord`, `SponsorEntry`, `SponsorLoyaltyLevel`, `SponsorLoyaltyXpMessage`, `CostCapComponent`, `CostCapTeam`, `CostCapSeason` |
| structs (entrants, read) | `EntrantRecord` (drivers lane) |

Constants left in the code: element caps, enum values (transaction types, categories, reasons),
vtable slots (registry current season +0xf8, control-block free slot 1), the human flag
bit, hash-map control bytes, and the extent of the embedded RP wallet stores (only their first
qword is modelled).

## Components helper (`native/src/components.hpp`)

For every feature module. Finds career components through the game, never by fixed heap offsets.

```cpp
namespace ce::comp {
enum class Id { Career, CareerSetup, CareerTime, Schedule, Championship, ChampionshipStatsController,
                CareerBank, CostCap, CareerSponsors, MyTeamRF, MyTeamFacilities, MyTeamStats,
                MyTeamAttributes, OwnerPerks, OwnerSetup, CareerFacilities, ResourcePoints, Upgrades,
                EngineSuppliers, DriverStatistics, DriverCareerPlayerDriverStatistics,
                DriverContractsMyTeam, DriverCareerContractsManager, CareerDriverMarket,
                CareerDriverContractRenewal, TeamHQController, WeatherAndTimeOfDay, Count };

uintptr_t entity();                   // current championship entity, 0 when none
uintptr_t get(Id id);                 // that component on the entity, 0 when absent
const char *name(Id id);              // "CareerBankComponent"

struct Entrant {
    uintptr_t at;                     // the 0x88-byte entrant record
    uint32_t driver;                  // packed driver key (kind | stat id << 16)
    uint32_t team;                    // packed team key (kind | stat id << 16)
    bool human;
};
std::vector<Entrant> entrants();
bool player(Entrant &out);            // the single human entrant; false if none or more than one
int humans();                         // number of human entrants

bool readable(uintptr_t p, size_t n); // committed, readable memory (whole range)
bool in_image(uintptr_t p);           // inside F1_25.exe's mapped image
bool object_like(uintptr_t obj);      // readable and its first qword (vtable) is in the image
template <class T> bool read(uintptr_t a, T &out);
template <class S> const S *view(uintptr_t p);          // p as a KB struct when sizeof(S) bytes are readable
template <class S> bool array(const S *b, const S *e, size_t max, size_t &n); // [b,e) whole, readable, <= max
}
```

How it works: `entity()` is the core career library's `swofty::career::entity()` (the game's
`GameManager → GameModeContext → ChampionshipHolder → CareerEntity` chain), accepted when its
embedded component registry has a vtable in the image. `get` is `swofty::career::component_by_desc()`
with the component's KB descriptor: the entity's own registry get (`CareerEntity.component_registry`,
vtable slot `+0x10`), as the game's own components resolve their siblings. `readable`, `in_image` and
`object_like` are `swofty/mem.hpp`'s guarded checks. Mode follows from the components: `CareerBank` is
My Team only. Call it on the flow thread (from an op or a section).

KB bindings it needs (all resolve at addon init): the 25 `k_Career_<Class>ComponentDesc` data rows
and `k_Career_ChampionshipDesc` (one per `Id`), plus `vt_Career_Championship`. `get` returns 0 when the entity has no such
component (for example `CareerBank` in Driver Career). Each miss is logged once per entity:
`components: <name> not found (entity …, desc …; registry get returned …, vtable …, type desc …)`.
It shows what the registry get itself returned, before the object check, plus that pointer's vtable
and the descriptor its GetType slot (`+0x28`, a `lea rax, [desc]; ret` in every component) names.
A returned value of 0 means the component is not on the entity. A non-zero value means the checks
rejected what the game returned.

`Id::Championship` is special: no game code looks the championship up by key (the registry indexes
components by their type chain and stops at the `ChampionshipComponent` base). `get(Id::Championship)`
returns the first of the entity, the `+0x88` holder and a registry get with the concrete `Championship`
type (`k_Career_ChampionshipDesc`) whose vtable is `vt_Career_Championship`, logs once which one it
was (`components: Championship found as the …`), and returns 0 otherwise (the log then lists the
three candidates and their vtables).
