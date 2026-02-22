> Farmctl Notes
>
> - `farmctl` is the single installer entrypoint for controller bundles.
> - Bundles are local-path DMGs; `farmctl` should mount via `hdiutil` and validate manifest checksums.
> - Keep install/upgrade/rollback idempotent and safe; write release state under the install root.
> - Prefer Rust for all new functionality in this directory.
