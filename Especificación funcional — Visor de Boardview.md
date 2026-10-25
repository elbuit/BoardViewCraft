# Especificación funcional — Visor de Boardview

Oct 25, 2026 · @Toni Cuñat

## 1. Introducción

Este documento define las funcionalidades de un visor de boardview para reparación electrónica, en dos ediciones: **Básica** y **Pro**. Reúne la suma de las funciones de los visores existentes (OpenBoardView, BoardViewer, Allegro Free Physical Viewer, Landrex, FlexBV, Phoneboard Pro y suites por suscripción como ZXW, XinZhiZao (XZZ), Wuxinji, Borneo o JCID) para que un equipo de desarrollo pueda implementarlo.

**Objetivo.** Permitir a un técnico localizar componentes, seguir señales y diagnosticar averías en placas de portátiles, móviles, tablets y placas base, comparando con valores de referencia y con el esquemático.

**Usuarios.** Técnicos de reparación a nivel de componente (microsoldadura), talleres y formadores.

**Convenciones.** Cada requisito tiene un identificador (p. ej. VIS-03), una prioridad (**Alta** = imprescindible para el MVP, **Media** = versión 1, **Baja** = posterior) y la edición en la que se incluye (**Básica** o **Pro**; lo que está en Básica también está en Pro).

**Glosario**

| Término | Significado |
| --- | --- |
| Boardview | Archivo con la posición física de componentes, pines y conexiones de una placa |
| Net | Línea eléctrica que une varios pines (p. ej. PP3V3\_S5, GND) |
| Pin / pad | Punto de conexión de un componente con la placa |
| Designador | Nombre del componente en la placa (U7400, C5120, R2310) |
| Top / Bottom | Cara superior e inferior de la placa |
| Modo diodo | Medición con multímetro que da una caída de tensión característica por línea |
| Esquemático | Plano eléctrico de la placa, normalmente en PDF |

## 2. Formatos de archivo

El visor debe abrir los formatos de boardview más extendidos y detectar el formato por contenido, no solo por extensión. La lista exacta de formatos y su estructura interna debe confirmarse con archivos de prueba reales antes de implementar cada parser.

| ID | Requisito | Prioridad | Edición |
| --- | --- | --- | --- |
| FMT-01 | Abrir .brd (variantes de texto y binarias) | Alta | Básica |
| FMT-02 | Abrir .bdv, .bv, .bvr, .cad (Samsung), .asc | Alta | Básica |
| FMT-03 | Abrir .fz (con su clave de descifrado) y .tvw, si es legalmente posible | Media | Pro |
| FMT-04 | Abrir .gr, .cst, .f2b y otros formatos menos comunes | Baja | Pro |
| FMT-05 | Detección automática del formato por cabecera y contenido | Alta | Básica |
| FMT-06 | Arrastrar y soltar archivos sobre la ventana; abrir desde el explorador del sistema (asociación de extensiones) | Alta | Básica |
| FMT-07 | Lista de archivos recientes | Alta | Básica |
| FMT-08 | Mensajes de error claros si el archivo está dañado o no es compatible, indicando el motivo | Alta | Básica |
| FMT-09 | Abrir archivos comprimidos (.zip, .rar, .7z) sin descomprimir a mano | Media | Pro |
| FMT-10 | Exportar la placa a un formato propio abierto (p. ej. JSON) con componentes, pines, nets y contorno; convertir formatos propietarios a ese formato | Baja | Pro |
| FMT-11 | Abrir .brd cifrados de fabricantes (Asus, Apple), si es legalmente posible | Media | Básica |
| FMT-12 | Leer el número de pieza (part number) y el valor de cada componente cuando el archivo los incluya | Media | Básica |

**Modelo de datos común.** Todos los parsers deben convertir el archivo a un modelo interno único: contorno de placa, serigrafía, componentes (designador, cara, posición, rotación, huella, número de pieza, valor), pines (número, posición, net, cara, radio, marca de pin 1) y nets (nombre, lista de pines). Si el archivo lo incluye, también capas de cobre, pistas y vías (pasantes, ciegas y enterradas) con las capas que unen. El resto del programa trabaja solo sobre ese modelo.

## 3. Visualización y navegación

La placa debe moverse con fluidez (objetivo: 60 fps) incluso con 10.000 componentes y 50.000 pines, usando renderizado por GPU.

| ID | Requisito | Prioridad | Edición |
| --- | --- | --- | --- |
| VIS-01 | Zoom con rueda del ratón centrado en el cursor, y con teclado (+/−), sin pérdida de calidad (render vectorial) | Alta | Básica |
| VIS-02 | Desplazamiento (pan) arrastrando y con flechas del teclado | Alta | Básica |
| VIS-03 | Ajustar la placa a la ventana (fit) con un botón o tecla | Alta | Básica |
| VIS-04 | Alternar rápidamente entre cara Top (A) y Bottom (B) | Alta | Básica |
| VIS-05 | Rotar la vista en pasos de 90° y en ángulo libre | Alta | Básica |
| VIS-06 | Mostrar contorno de placa, serigrafía, huellas (footprints) de componentes, pads y designadores | Alta | Básica |
| VIS-07 | Mostrar u ocultar capas: contorno, serigrafía, componentes, pines, textos, vías | Media | Básica |
| VIS-08 | Modo rayos X: Top y Bottom superpuestas con transparencia ajustable, para ver qué hay justo debajo de un IC en la otra cara; también vista lado a lado | Media | Pro |
| VIS-09 | Formas de pin realistas (redondo, cuadrado, BGA) y tamaño real de los pads | Media | Pro |
| VIS-10 | Tema claro y oscuro; colores configurables para nets, pines y componentes | Media | Básica |
| VIS-11 | Etiquetas que se escalan con el zoom y se ocultan cuando se solapan | Media | Básica |
| VIS-12 | Minimapa con la posición de la vista actual sobre la placa | Baja | Pro |
| VIS-13 | Superponer una foto real en alta resolución de la placa, debajo o encima de los vectores, con opacidad ajustable y alineada por 3–4 puntos de referencia | Media | Pro |
| VIS-14 | Regla para medir distancias en mm o mil entre dos puntos | Baja | Pro |
| VIS-15 | Modo espejo horizontal y vertical independientes, para que la pantalla coincida con la orientación física de la placa en el microscopio | Alta | Básica |
| VIS-16 | Indicador distintivo del pin 1 (forma o color) en ICs, conectores y componentes polarizados | Alta | Básica |

### 3.1 Capas internas de cobre

La Básica solo muestra qué pads están conectados; la Pro muestra además el recorrido físico de las pistas por cada capa, cuando el archivo trae esa información. Sirve para raspar capas, reconstruir pistas rotas por corrosión y evitar dañar líneas al hacer reballing o cortes.

| ID | Requisito | Prioridad | Edición |
| --- | --- | --- | --- |
| CAP-01 | Dibujar las pistas de cobre de cada capa (Layer 1 a 8 o más) | Media | Pro |
| CAP-02 | Selector de capas visibles, con color y transparencia por capa | Media | Pro |
| CAP-03 | Mostrar vías pasantes, ciegas y enterradas, indicando qué capas une cada una | Media | Pro |
| CAP-04 | Al resaltar una net, iluminar también su recorrido de pistas y vías por todas las capas | Media | Pro |
| CAP-05 | Importar el trazado por capas desde formatos que lo incluyen (.fz completos, bitmaps tipo ZXW/XZZ), si es legalmente posible | Baja | Pro |

## 4. Búsqueda y filtros

La búsqueda debe responder en menos de 100 ms y llevar la vista al resultado automáticamente.

| ID | Requisito | Prioridad | Edición |
| --- | --- | --- | --- |
| BUS-01 | Buscar componente por designador (U7400) con centrado y resaltado | Alta | Básica |
| BUS-02 | Buscar net por nombre (PP3V3\_S5, I2C\_SDA) | Alta | Básica |
| BUS-03 | Buscar pin concreto (U7400.12 o U7400 pin 12) | Alta | Básica |
| BUS-04 | Autocompletado mientras se escribe, sin distinguir mayúsculas | Alta | Básica |
| BUS-05 | Comodines y búsqueda parcial (PP3V3\*, \*I2C\*SCL\*) | Media | Básica |
| BUS-06 | Buscar varios elementos a la vez y resaltarlos con colores distintos | Media | Pro |
| BUS-07 | Panel lateral con lista de componentes y lista de nets, ordenables y filtrables | Alta | Básica |
| BUS-08 | Filtrar por tipo de componente según prefijo (U, Q, C, R, L, D, F, J) | Media | Pro |
| BUS-09 | Filtrar por cara (Top / Bottom) | Media | Básica |
| BUS-10 | Historial de navegación (breadcrumbs): lista de nets y componentes visitados, con atrás/adelante paso a paso como en un navegador | Media | Básica |
| BUS-11 | Buscar por valor o número de pieza (100nF, 0402, código del chip), si el archivo lo trae | Baja | Pro |
| BUS-12 | Búsqueda con expresiones regulares | Media | Pro |
| BUS-13 | Búsqueda por proximidad: listar los componentes de una net que estén a menos de X mm de un componente dado | Baja | Pro |

## 5. Análisis de nets y pines

El resaltado de net es la función más usada: un clic en un pin debe iluminar al instante todos los pines de esa net en ambas caras.

| ID | Requisito | Prioridad | Edición |
| --- | --- | --- | --- |
| NET-01 | Clic en un pin → resaltar en color de alto contraste todos los pines, pads y test points de su net, en Top y Bottom | Alta | Básica |
| NET-02 | Panel de información del pin: componente, número de pin, net, cara, coordenadas | Alta | Básica |
| NET-03 | Panel de información del componente: designador, cara, número de pieza y valor (si existen), nº de pines, lista de pines con su net | Alta | Básica |
| NET-04 | Lista de conexiones de una net (todos los componentes y pines que toca) con clic para saltar a cada uno | Alta | Básica |
| NET-05 | Identificar GND con un color reservado y atenuarlo para que no sature la vista | Alta | Básica |
| NET-06 | Marcar automáticamente nets de alimentación (prefijos PP, VCC, VDD, VBAT, etc.) con color propio | Media | Pro |
| NET-07 | Nets relacionadas: seguir una señal a través de resistencias de 0 Ω, bobinas y fusibles hasta la net siguiente | Media | Pro |
| NET-08 | Mostrar las líneas de conexión entre pines de una net (ratsnest) | Media | Pro |
| NET-09 | Marcar pines sin conexión (NC) o mecánicos y pines de prueba (test points) de forma diferenciada | Media | Básica |
| NET-10 | Fijar varias nets resaltadas a la vez, cada una con su color (p. ej. SDA azul, SCL verde, VCC rojo), sin que se borren al hacer clic en otro sitio | Media | Pro |
| NET-11 | Tooltip al pasar el ratón sobre un pin, con su net y valor de referencia si existe | Media | Básica |

## 6. Integración con el esquemático (PDF)

Es la función que más diferencia la edición Pro: vincula cada componente y net del boardview con su lugar en el esquemático, en los dos sentidos.

| ID | Requisito | Prioridad | Edición |
| --- | --- | --- | --- |
| SCH-01 | Visor de PDF integrado, con zoom, búsqueda de texto y lista de páginas | Alta | Pro |
| SCH-02 | Asociar un PDF a un boardview y recordar la asociación | Alta | Pro |
| SCH-03 | Clic en componente o net del boardview → saltar a su página en el PDF, hacer zoom y resaltarla | Alta | Pro |
| SCH-04 | Clic en texto del PDF (designador o net) → localizar y resaltar en el boardview | Alta | Pro |
| SCH-05 | Recorrer todas las apariciones de un elemento en el PDF (anterior / siguiente) | Media | Pro |
| SCH-06 | Pantalla dividida boardview + esquemático o diagrama de bloques, con el foco sincronizado; también en dos monitores | Media | Pro |
| SCH-07 | Indexar el PDF al abrirlo (texto con coordenadas) para que los saltos sean instantáneos | Alta | Pro |
| SCH-08 | Avisar si un designador o net del boardview no aparece en el PDF | Baja | Pro |
| SCH-09 | Soporte de PDF escaneados mediante OCR | Baja | Pro |
| SCH-10 | Búsqueda unificada: un solo buscador que encuentra el término a la vez en la placa y en el esquemático | Media | Pro |
| SCH-11 | Sincronizar con un visor de PDF externo (SumatraPDF, Acrobat) mediante comunicación entre procesos | Baja | Pro |

**Nota técnica.** El enlace se basa en extraer el texto del PDF con su posición y cruzarlo con los nombres del modelo de datos. Hay que tolerar diferencias de formato (espacios, guiones, mayúsculas) entre ambos archivos.

## 7. Valores de medición de referencia

Cada pin o net puede llevar valores medidos en una placa sana (modo diodo, resistencia a masa, voltaje), para que el técnico compare con su placa averiada.

| ID | Requisito | Prioridad | Edición |
| --- | --- | --- | --- |
| MED-01 | Mostrar valores de referencia por net o pin: diodo (mV), resistencia (Ω) y voltaje (V) por estado de encendido | Alta | Pro |
| MED-02 | Leer los valores que algunos formatos ya incluyen dentro del archivo | Alta | Pro |
| MED-03 | Importar valores desde CSV (net, diodo, resistencia, voltaje, notas) | Media | Pro |
| MED-04 | Que el usuario introduzca y edite sus propias mediciones por pin o net | Alta | Pro |
| MED-05 | Comparar medición del usuario con la de referencia y colorear según desviación (verde dentro de tolerancia, rojo fuera) | Media | Pro |
| MED-06 | Tolerancia configurable (p. ej. ±10 %) | Media | Pro |
| MED-07 | Distinguir estados: sin alimentación, standby (G3/S5) y encendido (S0) | Media | Pro |
| MED-08 | Mapa de calor de la placa según las desviaciones medidas | Baja | Pro |
| MED-09 | Compartir valores medidos con la comunidad (opt-in, anónimo) para ampliar la base de referencia | Baja | Pro |

## 8. Anotaciones, marcadores y sesiones

Las anotaciones se guardan en un archivo aparte (no modifican el boardview original) y se vuelven a cargar al abrir la misma placa.

| ID | Requisito | Prioridad | Edición |
| --- | --- | --- | --- |
| ANO-01 | Notas de texto ancladas a un componente, pin, net o coordenada de la placa (p. ej. «Aquí reconstruí pista hacia R204») | Media | Básica |
| ANO-02 | Marcadores y favoritos con nombre para volver a una zona, componente o net | Media | Básica |
| ANO-03 | Marcar componentes con estado y color: desoldado/retirado, sospechoso/dañado, reemplazado/verificado OK, no montado de fábrica (DNP) | Media | Pro |
| ANO-04 | Dibujo libre, flechas y texto sobre la placa | Baja | Pro |
| ANO-05 | Sesión de reparación: guardar placa, PDF, vista, anotaciones y mediciones juntas en un archivo de proyecto | Media | Pro |
| ANO-06 | Exportar las anotaciones y vistas como informe (PDF o PNG de alta resolución) para el cliente o el historial del taller | Baja | Pro |
| ANO-07 | Importar y exportar anotaciones para compartirlas con otro técnico | Baja | Pro |
| ANO-08 | Aviso al cerrar si quedan componentes marcados como retirados | Baja | Pro |

## 9. Biblioteca y base de datos

Las suites por suscripción incluyen una biblioteca en la nube de boardviews, esquemáticos y casos de reparación. Antes de construirla hay que resolver el origen legal de los archivos: los boardviews y esquemáticos son propiedad de los fabricantes y no pueden redistribuirse sin permiso.

| ID | Requisito | Prioridad | Edición |
| --- | --- | --- | --- |
| BIB-01 | Biblioteca local: indexar las carpetas del usuario y buscar placas por marca, modelo o código de placa (p. ej. 820-01700) | Media | Básica |
| BIB-02 | Asociar automáticamente boardview y PDF del mismo modelo por nombre o código de placa | Media | Pro |
| BIB-03 | Biblioteca en la nube con catálogo por marca → modelo → placa, descarga bajo demanda y caché local | Baja | Pro |
| BIB-04 | Fotos de alta resolución de placas reales, Top y Bottom | Baja | Pro |
| BIB-05 | Diagramas de bloques y secuencias de encendido por modelo | Baja | Pro |
| BIB-06 | Casos de reparación: síntoma, causa, componentes implicados y solución, enlazados a la placa | Baja | Pro |
| BIB-07 | Hojas de datos (datasheets) de chips enlazadas al componente | Baja | Pro |
| BIB-08 | Sincronizar anotaciones y mediciones entre equipos del mismo usuario | Baja | Pro |

## 10. Funciones complementarias

| ID | Requisito | Prioridad | Edición |
| --- | --- | --- | --- |
| COM-01 | Atajos de teclado configurables para todas las acciones frecuentes | Alta | Básica |
| COM-02 | Varias placas abiertas en pestañas | Media | Pro |
| COM-03 | Varias ventanas, para usar dos monitores | Media | Pro |
| COM-04 | Captura de pantalla de la vista actual (PNG) | Media | Básica |
| COM-05 | Imprimir o exportar a PDF la vista actual, con escala | Baja | Pro |
| COM-06 | Interfaz en varios idiomas (español e inglés como mínimo) | Alta | Básica |
| COM-07 | Pantalla táctil y gestos de trackpad (pellizcar para zoom, girar) | Media | Básica |
| COM-08 | Comprobación de actualizaciones al iniciar | Media | Básica |
| COM-09 | Comparar dos revisiones de la misma placa y mostrar componentes o nets distintos | Baja | Pro |
| COM-10 | Modo de uso con microscopio: interfaz a pantalla completa con controles grandes | Baja | Pro |

## 11. Ediciones Básica y Pro

La edición Básica sirve para *ver* la placa y seguir señales; la Pro añade lo necesario para *diagnosticar*. Todo lo de Básica está también en Pro.

| Área | Básica | Pro |
| --- | --- | --- |
| Formatos | Formatos comunes (.brd, incluidos cifrados de Asus/Apple, .bdv, .bv, .bvr, .cad, .asc) | Además .fz con clave, .tvw, comprimidos, formatos poco comunes y conversión a formato abierto |
| Visualización | Zoom vectorial, pan, Top/Bottom, espejo H/V, rotación, capas, pin 1, temas | Rayos X (Top/Bottom con transparencia), pads reales, minimapa, foto HD superpuesta, regla |
| Capas de cobre | — | Pistas por capa, vías pasantes, ciegas y enterradas, recorrido físico de nets |
| Búsqueda | Componente, net, pin, autocompletado, comodines, historial de navegación | Búsqueda múltiple con colores, regex, proximidad, filtro por tipo y por valor |
| Nets | Resaltado, info de pin y componente, lista de conexiones, GND, NC y test points | Nets de alimentación, nets relacionadas, ratsnest, varias nets fijadas con colores |
| Esquemático | — | Visor PDF integrado, vinculación bidireccional, pantalla dividida, búsqueda unificada, visor externo |
| Mediciones | — | Valores de referencia, mediciones propias, comparación y mapa de calor |
| Anotaciones | Notas ancladas, marcadores y favoritos | Estados de componente (retirado, sospechoso, OK, DNP), dibujo, sesiones, informes |
| Biblioteca | Indexado de carpetas locales | Biblioteca en la nube, fotos, casos, datasheets, sincronización |
| Extras | Atajos, capturas, idiomas, táctil | Pestañas, multimonitor, impresión, comparar revisiones |

**Licencias (propuesta a validar con negocio).** Básica gratuita o de pago único bajo. Pro por suscripción mensual o anual, o licencia perpetua con un año de actualizaciones. Activación por clave vinculada a un número limitado de equipos (p. ej. 2), con uso sin conexión durante al menos 30 días.

## 12. Requisitos no funcionales

| ID | Requisito |
| --- | --- |
| NFR-01 | Plataformas: Windows 10/11 (x64) como prioridad; macOS (Intel y Apple Silicon) y Linux en segunda fase |
| NFR-02 | Rendimiento: abrir un boardview de portátil en menos de 2 s; 60 fps al navegar; búsqueda en menos de 100 ms |
| NFR-03 | Memoria: menos de 500 MB con una placa grande y su PDF abiertos |
| NFR-04 | Funcionamiento sin conexión para todo excepto la biblioteca en la nube y la activación inicial |
| NFR-05 | Seguridad: los parsers deben resistir archivos malformados sin colgarse ni ejecutar código (pruebas con fuzzing) |
| NFR-06 | Privacidad: ninguna telemetría ni envío de datos sin consentimiento explícito |
| NFR-07 | Internacionalización: todos los textos en archivos de traducción; formato de números según idioma |
| NFR-08 | Accesibilidad: interfaz escalable (100–200 %), contraste suficiente, colores no como único indicador |
| NFR-09 | Legal: no distribuir boardviews ni esquemáticos de terceros sin licencia; revisar la legalidad de descifrar formatos protegidos |
| NFR-10 | Arquitectura: parsers como módulos independientes con pruebas automáticas por formato, para añadir formatos nuevos sin tocar el resto |

## 13. Fases y criterios de aceptación

Se propone construir en tres fases, cada una entregable por sí sola. Las prioridades de las tablas anteriores marcan qué entra en cada fase.

1. **MVP (requisitos de prioridad Alta de la edición Básica).** Modelo de datos común, parsers de formatos comunes, visualización, búsqueda básica y resaltado de net.
   - Aceptación: abre al menos 20 boardviews reales de distintas marcas sin errores; resaltar una net tarda menos de 50 ms; 60 fps con la placa más grande del conjunto de prueba.
2. **Versión 1 (Básica completa + núcleo Pro).** Prioridad Media de Básica, más integración con PDF (SCH-01 a SCH-04, SCH-07), valores de referencia (MED-01, MED-02, MED-04) y licencias.
   - Aceptación: en 10 pares boardview + esquemático de prueba, el salto entre ambos acierta en al menos el 95 % de los componentes; la activación Pro funciona con y sin conexión.
3. **Versión 2 (Pro completa).** Resto de requisitos Pro: biblioteca en la nube, sesiones, comparación de revisiones, mapa de calor, macOS y Linux.
   - Aceptación: definir con el equipo al cerrar la versión 1, según uso real y comentarios de usuarios.

**Preguntas abiertas**

- [ ] ¿La Básica será gratuita o de pago?
- [ ] ¿De dónde saldrán los archivos y valores de referencia de la biblioteca, y con qué permisos?
- [ ] ¿Qué formatos cifrados se pueden abrir legalmente en los países de venta?
- [ ] ¿Aplicación nativa (C++/Qt, Rust) o multiplataforma con web (Electron/Tauri + WebGL)?
