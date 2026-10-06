//! Writes `src/bindings.ts` from the registered commands. Run via `pnpm bindings`.

use specta_typescript::Typescript;

fn main() {
    let out = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/bindings.ts");
    anvil_lib::specta_builder()
        .export(Typescript::default(), out)
        .expect("failed to export bindings");
    println!("wrote {out}");
}
