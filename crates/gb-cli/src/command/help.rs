pub(crate) const RUN_HELP_TEXT: &str = concat!(
    "Usage:\n",
    "  gb-cli run <rom> [options]\n",
    "\n",
    "Options:\n",
    "  --model <DMG|MGB|LGB|CGB|AGB|SGB|SGB2> Select the console model/profile (default: DMG)\n",
    "  --revision <dmg-cpu-c|cpu-mgb|cpu-cgb-c|cpu-cgb-d|cpu-cgb-e|cpu-agb-a>\n",
    "                                         Select the active hardware revision for --model\n",
    "  --sgb-standard <ntsc|pal>             Select the original SGB video standard (requires --model SGB)\n",
    "  --startup <skip-boot|custom-boot|real-boot> Choose startup path (default: skip-boot)\n",
    "  --mode <strict|permissive|experimental> Set the compatibility policy (default: strict)\n",
    "  --boot-rom-dir <dir>                   Override the boot ROM directory root\n",
    "  --boot-rom-verify <off|warn|strict>    Control boot ROM SHA-256 verification (default: strict)\n",
    "  --test-runner                          Use host-light runner defaults: permissive mode, DMG grey palette, and no SGB border\n",
    "  --benchmark <path>                     Run one portable benchmark case TOML\n",
    "  --frames <n>                           Stop after <n> completed frames\n",
    "  --tcycles <n>                          Stop after <n> T-cycles\n",
    "                                         If neither limit is provided, direct boot stops after 120 completed frames\n",
    "                                         and real-boot stops after boot-ROM handoff plus 120 completed frames\n",
    "                                         with a 480-frame safety cap if handoff never arrives\n",
    "  --serial-stdout                        Stream completed serial bytes to stdout as they arrive\n",
    "  --serial-out <path>                    Save completed serial bytes to a file at the end of the run\n",
    "  --framebuffer-out <path>               Save the final framebuffer as PGM, or PNG when <path> ends in .png (SGB PNG uses 256x224 RGB555)\n",
    "  --border-off                           Hide the SGB/SGB2 host border for PNG framebuffer artifacts; ignored by other models\n",
    "  --palette <grey>                       Use the DMG grey framebuffer palette when --model DMG is active\n",
    "  --trace-out <path>                     Save the scheduler trace text for the run\n",
    "  --state-in <path>                      Restore a full-machine .gbstate after loading the ROM\n",
    "  --state-out <path>                     Save a full-machine .gbstate at the end of the run\n",
    "  --save-dir <dir>                       Load/save battery-backed cartridge persistence under this directory\n",
    "  --save-key <key>                       Override the derived save key (default: ROM stem)\n",
    "  --save-policy <manual|on-close|on-write>\n",
    "                                         Select automatic persistence behavior (default: on-close)\n",
);

pub(crate) const INSPECT_HELP_TEXT: &str = concat!(
    "Usage:\n",
    "  gb-cli inspect-rom <rom> [--mode <strict|permissive|experimental>]\n",
    "\n",
    "Options:\n",
    "  --mode <strict|permissive|experimental> Evaluate loader compatibility under the selected mode\n",
);

pub(crate) const SAVES_HELP_TEXT: &str = concat!(
    "Usage:\n",
    "  gb-cli saves export <rom> <out.sav> --save-dir <dir> [--save-key <key>]\n",
    "  gb-cli saves import <rom> <in.sav> --save-dir <dir> [--save-key <key>]\n",
    "\n",
    "Options:\n",
    "  --save-dir <dir>                       Directory containing gb-cycle cartridge save files\n",
    "  --save-key <key>                       Override the derived save key (default: ROM stem)\n",
);

pub(crate) fn general_help_text() -> &'static str {
    concat!(
        "Usage:\n",
        "  gb-cli run <rom> [options]\n",
        "  gb-cli inspect-rom <rom> [--mode <strict|permissive|experimental>]\n",
        "  gb-cli saves <export|import> <rom> <save.sav> --save-dir <dir> [--save-key <key>]\n",
        "\n",
        "Commands:\n",
        "  run         Execute one ROM with the headless runner\n",
        "  inspect-rom Parse the cartridge header and report mapper compatibility\n",
        "  saves       Convert gb-cycle cartridge persistence to or from external .sav files\n",
        "\n",
        "Run `gb-cli <command> --help` for command-specific options.\n",
    )
}
