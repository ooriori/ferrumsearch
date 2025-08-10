# FerrumSearch - High-Performance Search Engine in Rust

![FerrumSearch Logo](assets/logo.png) *(opcional)*

##  Descripción general

**FerrumSearch** es un motor de búsqueda desarrollado en **Rust** que combina altísimo rendimiento, seguridad y funcionalidades avanzadas:

- Búsqueda por relevancia con ranking **BM25**
- Búsqueda **difusa** para corregir errores ortográficos
- **Filtros** por metadata (p. ej.: categoría, año, dificultad)
- **Paginación** de resultados
- **Autocompletado** y **sugerencias**
- **Fragmentos destacados** (highlights) de texto
- Estadísticas del índice y funciones de mantenimiento
- Concurrente y seguro (usa `Arc<RwLock<>>`)

 

##  Características principales

| Funcionalidad               | Detalles |
|-----------------------------|----------|
| Indexación segura           | Añade y elimina documentos actualizando índices invertidos, frecuencias y longitudes. |
| Ranking BM25                | Fórmula estándar usada en motores como Elasticsearch, con control total sobre el cálculo. |
| Búsqueda difusa             | Corrige errores ortográficos con distancia de edición mínima (≤ 1). |
| Filtros por metadata        | Refina la búsqueda usando metadatos (por ejemplo: `year = 2024`). |
| Paginación                  | Controla página y tamaño de resultados (`page`, `per_page`). |
| Autocompletado & sugerencias | Basado en prefijos y coincidencias fuzzys. |
| Fragmentos destacados       | Muestra contextos relevantes en los resultados (highlights). |
| Estadísticas del índice     | Total de documentos, tamaño estimado, versión, última actualización. |
| Bulk import & limpieza      | Importa lotes de documentos o reinicia por completo el índice. |
| Concurrencia segura         | Uso de `Arc<RwLock<>>` para acceso concurrente sin riesgos. |

---

##  Cómo comenzar

Clona el repositorio:
```bash
git clone https://github.com/ooriori/ferrumsearch.git
cd ferrumsearch

Ejecuta el motor en modo demo:

cargo run

Verás búsquedas demo, estadísticas y autocompletado en consola.
Estructura del proyecto

│
├── Cargo.toml          ← Dependencias y metadatos del proyecto
├── README.md           ← Este archivo
└── src
    └── main.rs         ← Implementación de FerrumSearch, datos demo y tests

Extensiones recomendadas

    🚀 Exponer la API como REST (usando Axum o Warp)

    Persistencia en disco (para mantener el índice entre ejecuciones)

    Benchmarks y tests más extensos

    Dockerfile para despliegue sencillo en servidores o CI

    Dashboard web minimalista para búsqueda interactiva

    Soporte multilingüe y mejora de scoring (BM25+logística)

Contribuciones

¡Las contribuciones son más que bienvenidas! Para colaborar:

    Haz un fork ✂

    Crea una rama (feature/nombre)

    Realiza tus cambios y commitea (git commit -m "feat: descripción")

    Envía un pull request

Licencia & Autor

FerrumSearch está bajo licencia MIT.
Desarrollado por ooriori.
Contacto

¿Dudas, sugerencias o planes de colaboración? Contáctame en [senyassrcruzr@gmail.com].


---

###  Sugerencias para llevarlo aún más lejos

- Añade una imagen o logo con el nombre del proyecto e inclúyela en la sección principal.
- Incluye badges (estado de build, cobertura de tests, licencias, crates.io si lo publicas).
- Crea archivos `LICENSE` y `CONTRIBUTING.md` para profesionalizar aún más el repositorio.

¿Quieres que te ayude a generar también un **Dockerfile** o los badges para integrarlos directamente en el README?
::contentReference[oaicite:0]{index=0}
