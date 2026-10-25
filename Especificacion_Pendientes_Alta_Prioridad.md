# Requisitos Pendientes de Alta Prioridad — BoardViewCraft

Este documento recoge y desglosa las especificaciones extraídas de `Especificación funcional — Visor de Boardview.md` clasificadas con **Prioridad Alta** que aún **no están implementadas** en BoardViewCraft, estructuradas como hoja de ruta técnica para su desarrollo.

---

## 1. Formatos e Integración con Sistema de Archivos

| ID | Requisito | Edición | Estado Actual | Trabajo Pendiente |
|---|---|---|---|---|
| **FMT-06** | Arrastrar y soltar archivos sobre la ventana (Drag & Drop) y asociación de extensiones | Básica | No implementado en UI | Integrar `ctx.input(\|i\| i.raw.dropped_files)` en `crates/bvc-app/src/main.rs` para capturar archivos soltados sobre el canvas y abrirlos automáticamente sin abrir el diálogo modal. |
| **FMT-07** | Lista de archivos recientes | Básica | No implementado | Añadir historial persistente en `Preferences` (`recent_files: Vec<PathBuf>`) y un menú desplegable "Open Recent" en la barra superior. |

---

## 2. Visualización y Control de Entrada

| ID | Requisito | Edición | Estado Actual | Trabajo Pendiente |
|---|---|---|---|---|
| **VIS-01** | Zoom con teclado (`+` / `-`) | Básica | Solo rueda / presets | Registrar los atajos de teclado para teclado numérico y teclas `+` y `-` en el canvas principal para escalar suavemente la vista con teclado. |
| **VIS-02** | Desplazamiento (Pan) con flechas del teclado | Básica | Solo ratón | Añadir soporte para flechas direccionales (`Up`, `Down`, `Left`, `Right`) desplazando el centro del viewport en incrementos fijos o proporcionales a la escala. |

---

## 3. Integración con el Esquemático (PDF)

*Módulo principal de la edición Pro para vincular placa física y esquemático eléctrico.*

| ID | Requisito | Edición | Estado Actual | Trabajo Pendiente |
|---|---|---|---|---|
| **SCH-01** | Visor de PDF integrado con zoom, búsqueda y páginas | Pro | No implementado | Integrar backend de renderizado/lectura de PDF (p. ej. `pdfium-render` o librería afín en Rust/wgpu) con navegación de páginas y zoom. |
| **SCH-02** | Asociar un PDF a un boardview y recordar la asociación | Pro | No implementado | Guardar la ruta del archivo PDF asociado al hash o ruta del boardview en la configuración del usuario / base de datos local. |
| **SCH-03** | Clic en componente o net del boardview → saltar al PDF | Pro | No implementado | Al hacer clic sobre un componente o net seleccionada, buscar la coincidencia de texto en el índice del esquemático y enfocar la página/coordenada en el visor PDF. |
| **SCH-04** | Clic en texto del PDF → localizar y resaltar en el boardview | Pro | No implementado | Selección de texto o click-target en la vista PDF que dispare la selección activa en el canvas del boardview (`selected_part_idx` o `selected_net`). |
| **SCH-07** | Indexar el PDF al abrirlo (texto con coordenadas) | Pro | No implementado | Extracción en segundo plano de todos los tokens de texto, designadores y nets del PDF mapeados a páginas y cajas de colisión para que los saltos sean instantáneos (<50 ms). |

---

## 4. Valores de Medición de Referencia

*Diagnóstico de averías mediante comparación de modo diodo, impedancia y tensiones.*

| ID | Requisito | Edición | Estado Actual | Trabajo Pendiente |
|---|---|---|---|---|
| **MED-01** | Mostrar valores de referencia por net o pin (diodo mV, resistencia Ω, voltaje V) | Pro | No implementado | Extender el modelo de datos (`Pin`, `Net`) con campos opcionales de referencia y mostrarlos en los paneles de detalle del pin y tooltip. |
| **MED-02** | Leer valores de referencia incluidos en formatos que lo soportan | Pro | No implementado | Soporte en parsers compatibles (p. ej. variantes específicas de `.fz` o `.cad`) para volcar mediciones embebidas directamente al modelo. |
| **MED-04** | Introducir y editar mediciones propias por pin o net | Pro | No implementado | Campo editable en el panel lateral (`Net Details` / `Pin Details`) para que el técnico anote sus lecturas reales en placa y se guarden en archivo de sesión/anotaciones. |

---

## 5. Configuración y Accesibilidad

| ID | Requisito | Edición | Estado Actual | Trabajo Pendiente |
|---|---|---|---|---|
| **COM-01** | Atajos de teclado configurables para acciones frecuentes | Básica | Cableados fijos | Crear pestaña o diálogo de atajos en `Preferences` que permita reasignar combinaciones de teclas (rotar, cambiar de cara, espejo, fit, zoom). |

---

## Resumen de Prioridades para la Siguiente Fase (Roadmap Inmediato)

1. **Quick Wins (Edición Básica):**
   - **FMT-06 (Drag & Drop)** y **FMT-07 (Archivos Recientes)**: Muy baja complejidad, enorme impacto inmediato en ergonomía.
   - **VIS-01 / VIS-02 (Zoom y Pan con teclado)**: Atajos directos sobre el canvas.
   - **COM-01 (Mapeo de atajos)**: Reasignación de teclas desde el panel de preferencias.

2. **Núcleo Técnico Mayor (Edición Pro):**
   - **Esquemáticos (SCH-01 .. SCH-07)**: Incorporación de librería PDF y sincronización bidireccional placa ↔ PDF.
   - **Mediciones (MED-01 .. MED-04)**: Capa de datos para modo diodo y lecturas de referencia.
