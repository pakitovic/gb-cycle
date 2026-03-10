# gb-cycle

Emulador de Game Boy centrado en exactitud de hardware y escrito en Rust.

La base temporal objetivo del core es `1 tick = 1 T-cycle`.
La base gráfica objetivo es un PPU `dot-by-dot` con `tile fetcher + pixel FIFO`.

## Objetivos

- Priorizar el comportamiento fiel al hardware real.
- Mantener un core portable y desacoplado de cualquier frontend.
- Facilitar validación con tests y ROMs de referencia.
- Construir el core desde el principio sobre una línea temporal por `T-cycle`, no por `M-cycle`.
- Modelar el PPU desde el principio como pipeline real, no como renderer por scanline.

## Estructura prevista

La documentación del proyecto recomienda una organización similar a esta:

```text
core/        Lógica pura de emulación
frontends/   CLI, escritorio o web
tests/       Tests de integración, ROMs y helpers
AI/          Arquitectura, reglas y documentación técnica
```

## Primeros pasos

Cuando se inicialice el proyecto Rust:

```bash
cargo build
cargo test
```

## Documentación

Antes de implementar subsistemas, consulta las guías en `AI/`:

- `AI/ARCHITECTURE.md`
- `AI/CODING-RULES.md`
- `AI/EXECUTION.md`
- `AI/hardware/*.md`
