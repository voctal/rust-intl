# rust_intl_macros

- One subdirectory per locale.
- One file per namespace, used as a key prefix: `errors.json` key `not_found` becomes `errors.not_found`.
- Nested objects flatten with dots: `{"a": {"b": "..."}}` in `settings.json` becomes `settings.a.b`.
- Every locale must define the same keys with the same arguments.
- JSON only for now.
