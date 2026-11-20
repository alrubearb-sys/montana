# Montaña: herramienta del tiempo para salir al monte

Este archivo es la única fuente de la idea del proyecto (antes estaba en Notion, ya borrada).

## Objetivo

Construir una herramienta en Rust que use en mi día a día, integrada en la barra de Omarchy. Es la
excusa para aprender Rust de verdad. Tiene que ser algo que use y me guste usar, y que me aporte
más que cualquier app del tiempo, incluido el widget de temperatura que ya trae Omarchy.

## Decisiones tomadas

- Lenguaje: Rust. No C++ (de C++ solo quiero nociones para leerlo y saber qué hace).
- Descartado: CLI de gastos (ya tengo el MCP de Caudal) y proyecto Monte Carlo (solo valdría para
  el CV, lo haría la IA y no lo usaría).

## Qué es

Herramienta de terminal en Rust, más un módulo de la barra de Omarchy, que dice si merece la pena
salir a la montaña. Pensada para esquí y también para escalada y alpinismo, con dos modos: esquí y
montaña.

Lo que la diferencia de una app del tiempo (la idea que más me gusta): comparar mis zonas favoritas
("¿cuál es mejor este sábado?") con un ranking según mis propios criterios. Ejemplo de zona: Bödele
(Vorarlberg, Austria).

Ideas para añadir después:

- Avisos cuando se cumplan mis reglas (por ejemplo, más de 20 cm de nieve nueva, poco viento y
  buena visibilidad).
- Cruzar fuentes: pronóstico, boletín de aludes del SLF y estado de pistas o webcams en una sola señal.

## Cómo trabajamos

El proyecto es para aprender Rust de verdad (ownership, tipos, errores, async, arquitectura).

- Alejandro escribe el modelo de datos y las firmas de las funciones.
- Claude Code guía paso a paso, explica cada decisión de diseño y los errores del compilador,
  revisa su código y rellena solo lo mecánico cuando él lo pida.
- No escribir módulos enteros por él ni adelantarse a la fase siguiente.
- Explicaciones simples y en castellano de España, una recomendación clara en vez de varias opciones.

## Reglas de código

Sacadas de `docs/buenas-practicas.md` (lo que hacen Zed, ruff, uv, rust-lang y otros).

1. Antes de dar un paso por terminado, ejecutar `cargo fmt`, `cargo clippy` con avisos como errores
   y `cargo test`, y enseñarme la orden y su salida. No dar por hecho que un aviso o un test roto ya
   estaba antes.
2. Errores del compilador: explicarme qué significan y por qué salen, y darme una pista. No
   arreglarlos por mí salvo que lo pida.
3. Nada de `unwrap()`, `expect()` ni `panic!` fuera de los tests. Los errores se propagan con
   `Result` y `?`, con un tipo de error propio.
4. Usar enums propios en vez de cadenas o booleanos, y `match` completos sin `_`. Si hay que
   silenciar un aviso de Clippy, usar `#[expect]` y explicarme por qué.
5. No añadir ni actualizar crates sin preguntarme antes.
6. En `ranking`, cada regla nueva empieza por un test que falla; los tests de `datos` usan JSON
   guardado de Open-Meteo y del SLF, sin red.
7. Cambios pequeños y solo lo pedido, sin funciones ni archivos extra. Comentarios solo para
   explicar el porqué, nunca sobre la conversación ni sobre la edición.
8. Añadir una regla a este archivo solo cuando un error se repita, y mantenerlo por debajo de 200
   líneas.

## Datos

- Open-Meteo: pronóstico por coordenadas y altitud, gratis y sin clave. Su API de geocodificación
  pasa el nombre de una zona a coordenadas.
- Boletín de aludes del SLF (datos abiertos).

Esquí: nieve caída, cota de nieve, viento en altura. Montaña: isoterma de cero grados, horas de
luz, probabilidad de tormenta, riesgo de aludes.

## Arquitectura prevista

- `modelo`: estructuras de zona y pronóstico, enums, tipos propios.
- `datos`: peticiones HTTP async y JSON con serde, errores con `Result`.
- `ranking`: puntuación de zonas según mis criterios.
- `salida`: CLI con clap y salida JSON (texto y tooltip) para la barra; el clic en la barra lanza
  otra acción.

## Fases

1. CLI que devuelve el pronóstico de una zona (por ejemplo Bödele, Vorarlberg).
2. Módulo de la barra con esa salida.
3. Ranking comparando varias zonas.
4. Opcional: mapa de Suiza clicable (ventana ligera con Tauri, Leaflet y OpenStreetMap, con Rust
   de backend).

## Notas del sistema

- La barra ya no es Waybar: Omarchy la cambió por su propio shell (Quickshell) con plugins. La
  fase 2 es un plugin de Omarchy (ver la skill `omarchy`, guía `plugins.md`), no un módulo de Waybar.
- La carpeta está en MEGA. `.megaignore` excluye `target/` para no sincronizar lo compilado; `.git`
  tampoco se sincroniza (lo excluye el `.megaignore` general). El historial de git vive solo en este
  equipo hasta que haya un remoto en GitHub.

## Pendiente de decidir

- Qué datos mostrar en la barra (Alejandro se fija durante un par de semanas en qué mira antes de salir).
- Lista de zonas favoritas y criterios del ranking.
