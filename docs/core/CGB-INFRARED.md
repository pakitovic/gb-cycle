# CGB infrared, Pokémon Pikachu 2, and GSC Mystery Gift

`gb-core` models CGB infrared as bus-owned `RP` state plus explicit optical topologies. Native CGB-to-CGB IR sessions route light between two independent `Machine` instances, while accessory sessions pair one CGB `Machine` with a protocol device that only injects external IR light into the sensor.

`gb-desktop` exposes CGB infrared through the `GBC IR` overlay submenu when `CONFIG -> SYSTEM -> MODEL GB COLOR` is active. The root label reports `IR: NONE`, `IR: SAME GAME`, `IR: SELECT GAME`, `IR: PIKACHU 2`, or `IR: MYSTERY GIFT`; the submenu marks the active mode with `✓`, keeps `HELPER ON/OFF` for the top-right IR timing helper, and disables save states / rewind while an IR session is active.

`IR -> SAME GAME` clones the loaded CGB ROM into a fresh second console with an isolated P2 save slot. `IR -> SELECT GAME` asks for a second CGB ROM and supports different Gold / Silver / Crystal cartridges on the two IR sides, matching Mystery Gift station and two-console flows without treating IR as a Game Link cable mode.

The native CGB-to-CGB infrared path has been locally tested successfully with Pokémon Gold / Silver / Crystal, Super Mario Bros. DX, Pokémon Trading Card Game, Donkey Kong Country, Pokémon Pinball, and Perfect Dark.

`IR -> PIKACHU 2` enables the Pokémon Pikachu Color / Pokémon Pikachu 2 GS / Pocket Pikachu Color accessory model for western Pokémon Gold, Silver, and Crystal. The implementation generates the PP2 Mystery Gift protocol rather than replaying external waveform data, acts as PP2 role A, mirrors the receiving game's supported western region code (`0x90`, `0x96`, `0x99`, `0x9A`, or `0x9F`), re-arms after each successful send, and currently leaves Japanese / Korean validation as future work.

The `PIKACHU 2` gift selector is enabled only after `PIKACHU 2 ✓` is active and cycles the documented watt tiers shown in the overlay.

| `WATTS` | `GIFT` |
| --- | --- |
| `1W` | `EON MAIL` |
| `100W` | `BERRY` |
| `200W` | `BITTER BERRY` |
| `300W` | `GREAT BALL` |
| `400W` | `MAX REPEL` |
| `500W` | `ETHER` |
| `600W` | `MIRACLEBERRY` |
| `700W` | `GOLD BERRY` |
| `800W` | `ELIXIR` |
| `900W` | `REVIVE` |
| `999W` | `RARE CANDY` |

`IR -> MYSTERY GIFT` enables a custom western Pokémon Gold / Silver / Crystal Mystery Gift sender. It uses the same generated role-A IR protocol helper as `PIKACHU 2`, sends only the first 20-byte payload with version `0x03`, ID `0x0000`, trainer name `GB-CYCLE`, western region auto-detection, and no Trainer House team payload. `GIFT ITEM` / `GIFT DECORATION` selects the payload type and the gift selector cycles the documented `0x00..=0x24` table by name only, such as `BERRY`, `EON MAIL`, `WEEDLE DOLL`, and `TENTACOOL DOLL`; long labels scroll in the same way as `PIKACHU 2`.

The custom Mystery Gift selector displays only these uppercase names, without the internal gift code:

| `GIFT ITEM` | `GIFT DECORATION` |
| --- | --- |
| `BERRY` | `JIGGLYPUFF DOLL` |
| `PRZCUREBERRY` | `POLIWAG DOLL` |
| `MINT BERRY` | `DIGLETT DOLL` |
| `ICE BERRY` | `STARYU DOLL` |
| `BURNT BERRY` | `MAGIKARP DOLL` |
| `PSNCUREBERRY` | `ODDISH DOLL` |
| `GUARD SPEC.` | `GENGAR DOLL` |
| `X DEFEND` | `SHELLDER DOLL` |
| `X ATTACK` | `GRIMER DOLL` |
| `BITTER BERRY` | `VOLTORB DOLL` |
| `DIRE HIT` | `CLEFAIRY POSTER` |
| `X SPECIAL` | `JIGGLYPUFF POSTER` |
| `X ACCURACY` | `SUPER NES` |
| `EON MAIL` | `WEEDLE DOLL` |
| `MORPH MAIL` | `GEODUDE DOLL` |
| `MUSIC MAIL` | `MACHOP DOLL` |
| `MIRACLEBERRY` | `MAGNA PLANT` |
| `GOLD BERRY` | `TROPIC PLANT` |
| `REVIVE` | `NES` |
| `GREAT BALL` | `NINTENDO 64` |
| `SUPER REPEL` | `BULBASAUR DOLL` |
| `MAX REPEL` | `SQUIRTLE DOLL` |
| `ELIXIR` | `PINK BED` |
| `ETHER` | `POLKADOT BED` |
| `WATER STONE` | `RED CARPET` |
| `FIRE STONE` | `BLUE CARPET` |
| `LEAF STONE` | `YELLOW CARPET` |
| `THUNDERSTONE` | `GREEN CARPET` |
| `MAX ETHER` | `JUMBO PLANT` |
| `MAX ELIXIR` | `VIRTUAL BOY` |
| `MAX REVIVE` | `BIG ONIX DOLL` |
| `SCOPE LENS` | `PIKACHU POSTER` |
| `HP UP` | `BIG LAPRAS DOLL` |
| `PP UP` | `SURF PIKACHU DOLL` |
| `RARE CANDY` | `PIKACHU BED` |
| `BLUESKY MAIL` | `UNOWN DOLL` |
| `MIRAGE MAIL` | `TENTACOOL DOLL` |
