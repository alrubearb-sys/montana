# Montaña: herramienta del tiempo para salir al monte

Este archivo es la única fuente de la idea del proyecto (antes estaba en Notion, ya borrada).

## Objetivo

Construir una herramienta en Rust que use en mi día a día, integrada en la barra de Omarchy. Es la
excusa para aprender Rust de verdad. Tiene que ser algo que use y me guste usar, y que me aporte
más que cualquier app del tiempo, incluido el widget de temperatura que ya trae Omarchy.

## Decisiones tomadas

- Rápido y con poca RAM: es la razón de usar Rust. Nada de procesos siempre abiertos si no hacen
  falta, ni navegadores incrustados (Tauri, Electron) en la barra. Medir antes de elegir.
- Lenguaje: Rust. No C++ (de C++ solo quiero nociones para leerlo y saber qué hace).
- Descartado: CLI de gastos (ya tengo el MCP de Caudal) y proyecto Monte Carlo (solo valdría para
  el CV, lo haría la IA y no lo usaría).

## Qué es

Herramienta de terminal en Rust, más un módulo de la barra de Omarchy, que dice si merece la pena
salir a la montaña. Pensada para esquí y también para escalada y alpinismo, con dos modos: esquí y
montaña.

Se abre con un clic en la barra, a la derecha del tiempo de Omarchy, y despliega un panel con los
datos y una gráfica por horas. En el Mac hará lo mismo desde la barra de menús, con el look de Omarchy.

Lo que la diferencia de una app del tiempo (la idea que más me gusta): comparar mis zonas favoritas
("¿cuál es mejor este sábado?") con un ranking según mis propios criterios. Ejemplo de zona: Bödele
(Vorarlberg, Austria).

Ideas para añadir después:

- Avisos cuando se cumplan mis reglas (por ejemplo, más de 20 cm de nieve nueva, poco viento y
  buena visibilidad).
- Cruzar fuentes: pronóstico, boletín de aludes del SLF y estado de pistas o webcams en una sola señal.

## Cómo trabajamos

El proyecto es para aprender Rust de verdad (ownership, tipos, errores, async, arquitectura).

- Claude Code escribe el código. Alejandro toma las decisiones, lee el código y lo entiende.
- Antes de cada paso, plantear las decisiones que le tocan, con una recomendación. Después,
  explicar el código nuevo por partes y enlazarlo con lo que ha leído del libro.
- Pasos pequeños, para que pueda leerlos, y sin adelantarse a la fase siguiente.
- La aplicación va entera en inglés: código, nombres, comentarios y lo que ve el usuario.
- Explicaciones simples y en castellano de España, una recomendación clara en vez de varias opciones.

## Reglas de código

Sacadas de `docs/buenas-practicas.md` (lo que hacen Zed, ruff, uv, rust-lang y otros).

1. Antes de dar un paso por terminado, ejecutar `cargo fmt`, `cargo clippy` con avisos como errores
   y `cargo test`, y enseñarme la orden y su salida. No dar por hecho que un aviso o un test roto ya
   estaba antes.
2. Errores del compilador: arreglarlos y, si enseñan algo de Rust, explicar qué significaban y por
   qué salían.
3. Nada de `unwrap()`, `expect()` ni `panic!` fuera de los tests. Los errores se propagan con
   `Result` y `?`, con un tipo de error propio.
4. Usar enums propios en vez de cadenas o booleanos, y `match` completos sin `_`. Si hay que
   silenciar un aviso de Clippy, usar `#[expect]` y explicarme por qué.
5. No añadir ni actualizar crates sin preguntarme antes.
6. En `ranking`, cada regla nueva empieza por un test que falla; los tests de `data` usan JSON
   guardado de Open-Meteo y del SLF, sin red.
7. Cambios pequeños y solo lo pedido, sin funciones ni archivos extra. Comentarios solo para
   explicar el porqué, nunca sobre la conversación ni sobre la edición.
8. Añadir una regla a este archivo solo cuando un error se repita, y mantenerlo por debajo de 200
   líneas.

## Datos

Cobertura: toda Europa, sin base de datos propia. El usuario busca o hace clic igual en todas partes
y el programa elige la fuente por detrás. Solo se guardan las zonas favoritas.

- Tiempo: Open-Meteo, por coordenadas y altitud, gratis y sin clave.
- Viento: el de 10 m se queda muy corto en las cimas (Aneto, media de un día: 21,7 km/h frente a 65,1
  en aire libre). Para decidir, usar `high_wind`, interpolado entre niveles de presión.
- Espesor de nieve: solo orientativo (Aneto, 1 cm a 3404 m). Buscar otra fuente después del panel.
- Buscar un sitio: Nominatim público, decidido por Alejandro el 2026-11-22 para uso personal. Respetar
  su política (https://operations.osmfoundation.org/policies/nominatim/): 1 petición por segundo,
  User-Agent propio, guardar resultados, citar la fuente, nada de autocompletar y dirección
  configurable. Con más usuarios hay que cambiar de servicio. No usar el buscador de Open-Meteo: con
  "Bödele" da un pico de Liechtenstein.
- Altitud: la de OpenStreetMap si el sitio la trae; si no, swisstopo en Suiza y, de momento,
  Open-Meteo en el resto (terrain tiles de AWS es más precisa, pero pide un crate para leer PNG).
  Open-Meteo se queda de 14 a 215 m por debajo en los picos probados.
- Mapas: swisstopo (CH), basemap.at (AT), TopPlusOpen (DE), IGN (FR y ES) y OpenTopoMap (resto).
- Cota de nieve: en los Pirineos (Aneto) Open-Meteo no la da en ninguna hora.
- Aludes: SLF solo cubre Suiza; avalanche.report da regiones de Austria e Italia. Sin fuente aún para
  Francia y los Pirineos.
- Citar las fuentes y leer sus condiciones antes de publicar la aplicación.

Esquí: nieve caída, cota de nieve, viento en altura. Montaña: isoterma de cero grados, horas de
luz, probabilidad de tormenta, riesgo de aludes.

## Arquitectura prevista

- `model`: estructuras de zona y pronóstico, enums, tipos propios.
- `data`: peticiones HTTP async y JSON con serde, errores con `Result`.
- `ranking`: puntuación de zonas según mis criterios.
- `output`: CLI con clap y salida JSON. Es el motor común: los paneles solo pintan ese JSON.
- Panel de Omarchy: plugin QML (`bar-widget` con panel) que corre dentro del shell y llama al motor.
- Panel del Mac: app nativa en Rust, sin navegador. Candidata: Slint, que hay que medir antes.

## Fases

1. CLI que busca un sitio, lo guarda y devuelve su pronóstico, también en JSON.
2. Plugin de Omarchy: botón en la barra y panel con datos y gráfica.
3. Ranking comparando varias zonas.
4. Mapa para marcar zonas: clic izquierdo la cima y clic derecho la base (opcional). Se abre solo para
   añadir y se cierra. Nativo en Rust (candidata: `walkers`), midiendo la RAM.
5. App de barra para el Mac con el mismo panel y el look de Omarchy.

## Notas del sistema

- La barra ya no es Waybar: Omarchy la cambió por su propio shell (Quickshell) con plugins. La
  fase 2 es un plugin de Omarchy (ver la skill `omarchy`, guía `plugins.md`), no un módulo de Waybar.
- La carpeta está en MEGA. `.megaignore` excluye `target/` para no sincronizar lo compilado; `.git`
  tampoco se sincroniza (lo excluye el `.megaignore` general).
- Repositorio público: https://github.com/alrubearb-sys/montana (licencia MIT o Apache 2.0). El
  historial vive ahí. Los commits usan el correo noreply de GitHub, nunca el Gmail personal.
- El trabajo se organiza con issues y una milestone por fase. Para saber qué toca:
  `gh issue list -R alrubearb-sys/montana --milestone "Phase 1: CLI forecast for one area"`.
  Los commits que terminan una tarea llevan "closes #N". README e issues en inglés; este archivo y
  `docs/` en castellano.
- Permiso de Alejandro (2026-11-22): subir cada tarea terminada a `main` cuando fmt, clippy y test
  pasen. Los issues nuevos, los crates y cualquier otra cosa pública se siguen preguntando.

## Pendiente de decidir

- Qué datos mostrar en la barra (Alejandro se fija durante un par de semanas en qué mira antes de salir).
- Lista de zonas favoritas y criterios del ranking.
