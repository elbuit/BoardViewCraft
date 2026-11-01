# Requisitos Pendientes de Alta Prioridad — BoardViewCraft

Este documento recoge los requisitos de **Prioridad Alta** de `Especificación funcional — Visor de Boardview.md` y su estado de implementación en BoardViewCraft. Estado revisado contra el código el 2026-11-01.

---

## 1. Pendiente

| ID | Requisito | Edición | Estado Actual | Trabajo Pendiente |
|---|---|---|---|---|
| **COM-01** | Atajos de teclado configurables para acciones frecuentes | Básica | Cableados fijos en `crates/ui-egui/src/lib.rs` (gestión de hotkeys globales) | Crear pestaña o diálogo de atajos en `Preferences` que permita reasignar combinaciones de teclas (rotar, cambiar de cara, espejo, fit, zoom) y persistirlas. |

---

## 2. Implementado

| ID | Requisito | Edición | Implementación |
|---|---|---|---|
| **FMT-06** | Arrastrar y soltar archivos sobre la ventana | Básica | `dropped_files` en `crates/ui-egui/src/lib.rs` |
| **FMT-07** | Lista de archivos recientes | Básica | `recent_files` en `crates/ui-egui/src/preferences.rs` |
| **VIS-01** | Zoom con teclado (`+` / `-`) | Básica | `crates/ui-egui/src/canvas.rs` |
| **VIS-02** | Desplazamiento (pan) con flechas del teclado | Básica | `crates/ui-egui/src/canvas.rs` |
| **SCH-01** | Visor de PDF integrado con zoom, búsqueda y páginas | Pro | `crates/ui-egui/src/pdf.rs`, `crates/ui-egui/src/pdf_tab.rs` |
| **SCH-02** | Asociar un PDF a un boardview y recordar la asociación | Pro | Asociaciones placa → PDF en `Preferences` (`remember_schematic`) |
| **SCH-03** | Clic en componente del boardview → saltar al PDF | Pro | Búsqueda del designador en el visor PDF desde la selección de la placa |
| **SCH-04** | Clic en texto del PDF → localizar y resaltar en el boardview | Pro | Detección de la palabra bajo el clic (`word_at`) y selección en la placa |
| **SCH-07** | Indexar el PDF al abrirlo (texto con coordenadas) | Pro | Indexado de texto en segundo plano (`text_indexed`) |

---

## 3. Reclasificado a prioridad baja

MED-01, MED-02 y MED-04 pasan a prioridad **Baja** (decisión del 2026-11-01) y salen de esta hoja de ruta; siguen descritos en la sección 7 de la especificación funcional.

| ID | Requisito | Edición |
|---|---|---|
| **MED-01** | Mostrar valores de referencia por net o pin (diodo mV, resistencia Ω, voltaje V) | Pro |
| **MED-02** | Leer valores de referencia incluidos en formatos que lo soportan | Pro |
| **MED-04** | Introducir y editar mediciones propias por pin o net | Pro |

---

## Siguiente paso

1. **COM-01 (Mapeo de atajos)**: único requisito de prioridad alta pendiente.
