# Top 100 PICO-8 current-converter benchmark

Source: [Nerdy Teachers](https://nerdyteachers.com/PICO-8/Games/Top200/). Exactly frozen ranks 1–100; no substitutions or source edits.

Converter commit: `7ad2e3f36f068bc1f3b4b4c428e369e6401b58b8`. Firmware-build result: **2/100**.

AddressSanitizer and UndefinedBehaviorSanitizer remained enabled. Leak detection alone was disabled because LeakSanitizer cannot operate under this host sandbox’s ptrace; the superseded first attempt is retained privately and excluded from these results.

| Stage | Passed / 100 |
| --- | ---: |
| downloaded | 100 |
| decoded | 100 |
| translated | 7 |
| host_passed | 2 |
| firmware_built | 2 |

Translation means generated C. Host checks run 1,200 scripted frames with ASan/UBSan. Firmware builds target RP2040/Pico + Waveshare LCD 1.3 and enforce the existing memory/stack checks. No flashing or physical gameplay tests were performed. A successful build is not a complete compatibility or fidelity test. Unavailable downloads remain in the denominator and are listed separately. Multi-cartridge/export/include dependencies are not assembled; the first public Cart File in the ranked post is tested as-is.

| Rank | Game | Download | Decode | C | Host | UF2 | First blocker |
| ---: | --- | :---: | :---: | :---: | :---: | :---: | --- |
| 1 | Celeste Classic | yes | yes | yes | — | — | CARTRIDGE_ERROR: native closure pool exhausted in one frame frame=0 |
| 2 | Just One Boss | yes | yes | — | — | — | Conversion failed: line 1661: varargs are not supported yet |
| 3 | Celeste Classic 2 | yes | yes | — | — | — | Conversion failed: line 803: goto statements and labels are not supported yet |
| 4 | Birds With Guns | yes | yes | — | — | — | Conversion failed: conversion stopped: unsupported function/API 'poke' |
| 5 | Terra | yes | yes | — | — | — | Conversion failed: line 483: unsupported expression '@' |
| 6 | Picohot | yes | yes | — | — | — | Conversion failed: line 477: goto statements and labels are not supported yet |
| 7 | Poom | yes | yes | — | — | — | Conversion failed: line 19: numeric string escape must be between 1 and 255 in this subset |
| 8 | High Stakes | yes | yes | — | — | — | Conversion failed: conversion stopped: unsupported function/API 'cartdata' |
| 9 | The Lair | yes | yes | — | — | — | Conversion failed: line 127: varargs are not supported yet |
| 10 | Dank Tomb | yes | yes | — | — | — | Conversion failed: line 399: unsupported expression '@' |
| 11 | Ascent | yes | yes | — | — | — | Conversion failed: conversion stopped: unsupported function/API 'poke' |
| 12 | Buns: Bunny Survivor | yes | yes | — | — | — | Conversion failed: line 336: varargs are not supported yet |
| 13 | Porklike | yes | yes | — | — | — | Conversion failed: conversion stopped: unsupported function/API 'cartdata' |
| 14 | Pico Night Punkin | yes | yes | — | — | — | Conversion failed: conversion stopped: unsupported function/API 'poke' |
| 15 | Picoware | yes | yes | — | — | — | Conversion failed: line 173: varargs are not supported yet |
| 16 | Oblivion Eve | yes | yes | — | — | — | Conversion failed: line 341: numeric string escape must be between 1 and 255 in this subset |
| 17 | Mai-Chans Sweet Buns | yes | yes | — | — | — | Conversion failed: conversion stopped: at most 16 values per expression list are supported |
| 18 | Marble Merger | yes | yes | — | — | — | Conversion failed: line 368: expected assignment or function call |
| 19 | Dusk Child | yes | yes | yes | yes | yes |  |
| 20 | Fuz | yes | yes | — | — | — | Conversion failed: conversion stopped: unsupported function/API 'menuitem' |
| 21 | Islander | yes | yes | — | — | — | Conversion failed: conversion stopped: unsupported function/API 'menuitem' |
| 22 | Pico Zombie Garden | yes | yes | — | — | — | Conversion failed: conversion stopped: unsupported function/API 'poke' |
| 23 | Kalikan | yes | yes | — | — | — | Conversion failed: conversion stopped: unsupported function/API 'dset' |
| 24 | X-Zero | yes | yes | — | — | — | Conversion failed: conversion stopped: unsupported function/API 'menuitem' |
| 25 | Curse Of The Lich King | yes | yes | — | — | — | Conversion failed: conversion stopped: unsupported function/API 'flip' |
| 26 | Combo Pool | yes | yes | — | — | — | Conversion failed: conversion stopped: unsupported function/API 'menuitem' |
| 27 | Pico World Race | yes | yes | — | — | — | Conversion failed: line 358: varargs are not supported yet |
| 28 | Low Knight | yes | yes | — | — | — | Conversion failed: line 32: varargs are not supported yet |
| 29 | Mistigri | yes | yes | — | — | — | Conversion failed: conversion stopped: unsupported function/API 'cartdata' |
| 30 | Pico Tetris | yes | yes | — | — | — | Conversion failed: line 17: varargs are not supported yet |
| 31 | Pico Driller | yes | yes | — | — | — | Conversion failed: conversion stopped: unsupported function/API 'cartdata' |
| 32 | Into Ruins | yes | yes | — | — | — | Conversion failed: line 1480: numeric string escape must be between 1 and 255 in this subset |
| 33 | Pico Dino | yes | yes | — | — | — | Conversion failed: line 412: expected assignment or function call |
| 34 | Beckon The Hellspawn | yes | yes | — | — | — | Conversion failed: conversion stopped: unsupported function/API 'menuitem' |
| 35 | To A Starling | yes | yes | — | — | — | Conversion failed: conversion stopped: unsupported function/API 'cartdata' |
| 36 | Woodworm | yes | yes | — | — | — | Conversion failed: line 17: numeric string escape must be between 1 and 255 in this subset |
| 37 | BAS | yes | yes | — | — | — | Conversion failed: conversion stopped: unsupported function/API 'poke' |
| 38 | Praxis Fighter X | yes | yes | — | — | — | Conversion failed: line 15: numeric string escape must be between 1 and 255 in this subset |
| 39 | Golf Sunday | yes | yes | — | — | — | Conversion failed: conversion stopped: unsupported function/API 'cartdata' |
| 40 | Picoracer-2048 | yes | yes | — | — | — | Conversion failed: conversion stopped: unsupported function/API 'poke' |
| 41 | Shape Of Mind | yes | yes | — | — | — | Conversion failed: conversion stopped: unsupported function/API 'cartdata' |
| 42 | Pico Fox | yes | yes | yes | — | — | CARTRIDGE_ERROR: table pool exhausted frame=0 |
| 43 | Ufo Swamp Odyssey | yes | yes | — | — | — | Conversion failed: conversion stopped: unsupported function/API 'poke' |
| 44 | Demon Castle | yes | yes | — | — | — | Conversion failed: conversion stopped: unsupported function/API 'setmetatable' |
| 45 | Air Delivery | yes | yes | — | — | — | Conversion failed: conversion stopped: unsupported function/API 'memcpy' |
| 46 | Marballs 2 | yes | yes | — | — | — | Conversion failed: conversion stopped: unsupported function/API 'flip' |
| 47 | Suika Game Demake | yes | yes | — | — | — | Conversion failed: conversion stopped: at most 16 values per expression list are supported |
| 48 | Freezing Knights | yes | yes | — | — | — | Conversion failed: conversion stopped: unsupported function/API 'menuitem' |
| 49 | Nanoman | yes | yes | — | — | — | Conversion failed: conversion stopped: unsupported function/API 'flip' |
| 50 | The Lost Night | yes | yes | — | — | — | Conversion failed: conversion stopped: unsupported function/API 'menuitem' |
| 51 | Pico Arcade | yes | yes | — | — | — | Conversion failed: conversion stopped: unsupported function/API 'load' |
| 52 | Sonic | yes | yes | — | — | — | Conversion failed: line 1880: varargs are not supported yet |
| 53 | Build a Jetpack | yes | yes | — | — | — | Conversion failed: line 85: expected 'then', found 'do' |
| 54 | Low Mem Sky | yes | yes | — | — | — | Conversion failed: conversion stopped: unsupported function/API 'cartdata' |
| 55 | Little Dragon Adventure | yes | yes | — | — | — | Conversion failed: conversion stopped: unsupported function/API 'poke' |
| 56 | Pico Checkmate | yes | yes | — | — | — | Conversion failed: conversion stopped: at most 16 values per expression list are supported |
| 57 | Alpine Alpaca | yes | yes | — | — | — | Conversion failed: line 16: goto statements and labels are not supported yet |
| 58 | Trial Of The Sorcerer | yes | yes | — | — | — | Conversion failed: conversion stopped: unsupported function/API 'poke' |
| 59 | P.Craft | yes | yes | — | — | — | Conversion failed: conversion stopped: unsupported function/API 'reload' |
| 60 | Witchcraft TD | yes | yes | — | — | — | Conversion failed: conversion stopped: unsupported function/API 'run' |
| 61 | 8 Legs To Love | yes | yes | — | — | — | Conversion failed: conversion stopped: unsupported function/API 'cartdata' |
| 62 | Motion●Rec | yes | yes | — | — | — | Conversion failed: conversion stopped: unsupported function/API 'poke' |
| 63 | Hug Arena | yes | yes | yes | yes | yes |  |
| 64 | Rolly | yes | yes | — | — | — | Conversion failed: conversion stopped: unsupported function/API 'menuitem' |
| 65 | Guncho | yes | yes | — | — | — | Conversion failed: conversion stopped: unsupported function/API 'costatus' |
| 66 | X-Wing Vs. Tie Fighter | yes | yes | — | — | — | Conversion failed: line 1386: goto statements and labels are not supported yet |
| 67 | Delunky | yes | yes | — | — | — | Conversion failed: conversion stopped: unsupported function/API 'menuitem' |
| 68 | Pigments | yes | yes | — | — | — | Conversion failed: conversion stopped: unsupported function/API 'cartdata' |
| 69 | Cherry Bomb | yes | yes | — | — | — | Conversion failed: conversion stopped: unsupported function/API 'cartdata' |
| 70 | Pat Shooter | yes | yes | yes | — | — | CARTRIDGE_ERROR: number expected frame=11 |
| 71 | Breakout Hero | yes | yes | — | — | — | Conversion failed: conversion stopped: unsupported function/API 'cartdata' |
| 72 | Scrap Boy | yes | yes | — | — | — | Conversion failed: line 10: numeric string escape must be between 1 and 255 in this subset |
| 73 | Shadows Of Dunwich | yes | yes | — | — | — | Conversion failed: conversion stopped: unsupported function/API 'poke' |
| 74 | Zepton | yes | yes | — | — | — | Conversion failed: line 1048: goto statements and labels are not supported yet |
| 75 | Tetyis | yes | yes | yes | — | — | CARTRIDGE_ERROR: only draw palette remapping is supported frame=0 |
| 76 | Pico Tennis | yes | yes | — | — | — | Conversion failed: conversion stopped: unsupported function/API 'menuitem' |
| 77 | Driftmania | yes | yes | — | — | — | Conversion failed: line 5: numeric string escape must be between 1 and 255 in this subset |
| 78 | Wobblepaint | yes | yes | — | — | — | Conversion failed: conversion stopped: unsupported function/API 'poke' |
| 79 | Night Ride | yes | yes | — | — | — | Conversion failed: conversion stopped: unsupported function/API 'fillp' |
| 80 | The Tower Of Archeos | yes | yes | — | — | — | Conversion failed: conversion stopped: unsupported function/API 'cartdata' |
| 81 | Whiplash Taxi Co | yes | yes | — | — | — | Conversion failed: conversion stopped: unsupported function/API 'cartdata' |
| 82 | Shelled Shinobi | yes | yes | — | — | — | Conversion failed: conversion stopped: unsupported function/API 'cartdata' |
| 83 | Picokaiju | yes | yes | — | — | — | Conversion failed: conversion stopped: unsupported function/API 'menuitem' |
| 84 | Harold's Bad Day | yes | yes | — | — | — | Conversion failed: conversion stopped: unsupported function/API 'poke' |
| 85 | Cab Ride | yes | yes | — | — | — | Conversion failed: conversion stopped: unsupported function/API 'menuitem' |
| 86 | Puzzles of the Paladin | yes | yes | — | — | — | Conversion failed: conversion stopped: unsupported function/API 'cartdata' |
| 87 | Dinky Kong | yes | yes | — | — | — | Conversion failed: line 13: numeric string escape must be between 1 and 255 in this subset |
| 88 | Brutal Pico Race | yes | yes | — | — | — | Conversion failed: conversion stopped: at most 16 values per expression list are supported |
| 89 | Steel Surge: Revolution | yes | yes | — | — | — | Conversion failed: line 160: varargs are not supported yet |
| 90 | Cattle Crisis | yes | yes | — | — | — | Conversion failed: line 236: expected 'then', found 'do' |
| 91 | Dungeon | yes | yes | — | — | — | Conversion failed: conversion stopped: unsupported function/API 'poke' |
| 92 | Subsurface | yes | yes | — | — | — | Conversion failed: line 6: unsupported expression '=' |
| 93 | Mots 8-Ball Pool | yes | yes | — | — | — | Conversion failed: line 1322: unsupported expression '//' |
| 94 | R-Type | yes | yes | — | — | — | Conversion failed: line 6: numeric string escape must be between 1 and 255 in this subset |
| 95 | Villager | yes | yes | yes | — | — | CARTRIDGE_ERROR: number expected frame=0 |
| 96 | Pico Snail | yes | yes | — | — | — | Conversion failed: line 398: expected assignment or function call |
| 97 | Starjump | yes | yes | — | — | — | Conversion failed: conversion stopped: unsupported function/API 'setmetatable' |
| 98 | Super Disc Box | yes | yes | — | — | — | Conversion failed: line 218: unsupported expression '$' |
| 99 | Pico Monsters | yes | yes | — | — | — | Conversion failed: conversion stopped: unsupported function/API 'dset' |
| 100 | Return Of The Slimepires | yes | yes | — | — | — | Conversion failed: line 299: goto statements and labels are not supported yet |

## First-blocker groups

Each failed game contributes its first observed blocker only; removing it may expose further issues.

| First blocker | Games |
| --- | ---: |
| missing API: cartdata | 16 |
| missing API: poke | 14 |
| missing API: menuitem | 12 |
| other unsupported syntax | 10 |
| varargs | 9 |
| numeric string escapes including NUL | 9 |
| goto/labels | 6 |
| 16-value expression-list limit | 4 |
| missing API: flip | 3 |
| missing API: dset | 2 |
| missing API: setmetatable | 2 |
| CARTRIDGE_ERROR: native closure pool exhausted in one frame frame=0 | 1 |
| CARTRIDGE_ERROR: table pool exhausted frame=0 | 1 |
| missing API: memcpy | 1 |
| missing API: load | 1 |
| missing API: reload | 1 |
| missing API: run | 1 |
| missing API: costatus | 1 |
| CARTRIDGE_ERROR: number expected frame=11 | 1 |
| CARTRIDGE_ERROR: only draw palette remapping is supported frame=0 | 1 |
| missing API: fillp | 1 |
| CARTRIDGE_ERROR: number expected frame=0 | 1 |
