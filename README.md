# gb-cycle

Emulador de Game Boy centrado en exactitud de hardware y escrito en Rust.

La base temporal objetivo del core es `1 tick = 1 T-cycle`.
La base CPU objetivo es un core de `fetch / decode / execute` con accesos reales al bus.
La base gráfica objetivo es un PPU `dot-by-dot` con `tile fetcher + pixel FIFO`.

## Objetivos

- Priorizar el comportamiento fiel al hardware real.
- Mantener un core portable y desacoplado de cualquier frontend.
- Facilitar validación con tests y ROMs de referencia.
- Construir el core desde el principio sobre una línea temporal por `T-cycle`, no por `M-cycle`.
- Modelar la CPU desde el principio como flujo real de fetch/decode/execute, no como opcodes opacos con duración agregada.
- Modelar el PPU desde el principio como pipeline real, no como renderer por scanline.

## Estructura prevista

La siguiente distribución es solo una guía conceptual temprana.
La estructura canónica y sus límites de ownership están definidos en `AI/ARCHITECTURE.md`.
A medio plazo, la forma preferida es la familia `crates/gb-core`, `gb-cli`, `gb-desktop`, `gb-web` y utilidades relacionadas descrita allí; el esquema siguiente solo agrupa responsabilidades.

```text
core/         Lógica pura de emulación
frontends/    CLI, escritorio o web
tooling/      Runner de ROMs, debugger y utilidades
persistence/  Adaptadores de saves/RTC fuera del core
tests/        Tests de integración, ROMs y helpers
AI/           Arquitectura, roadmap y documentación técnica
```

## Primeros pasos

Cuando se inicialice el proyecto Rust:

```bash
cargo build
cargo test
```

## Documentación

Antes de implementar subsistemas, consulta las guías en `AI/`:

- `AI/index.md`
- `AI/ARCHITECTURE.md`
- `AI/CODING-RULES.md`
- `AI/EXECUTION.md`
- `AI/REFERENCES.md`
- `AI/ROADMAP.md`
- `AI/TESTING.md`
- `AI/TIMING-AND-ACCURACY.md`
- `AI/hardware/*.md`
- `AI/research/*.md`
