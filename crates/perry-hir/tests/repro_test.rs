// Narrowing a pre-existing compiler overflow that reproduces with `class A extends B`
// in the same module as a top-level `const x = fn()` call to an unannotated factory.
// NOT caused by Phase 1/Phase 4 — reproduces with both disabled. Left here as a
// repro for a separate follow-up; actual shape tests live in shape_inference.rs.

use perry_diagnostics::SourceCache;
use perry_hir::lower_module;
use perry_parser::parse_typescript_with_cache;

fn lower(src: &str) {
    let mut cache = SourceCache::new();
    let parsed = parse_typescript_with_cache(src, "test.ts", &mut cache).unwrap();
    let _ = lower_module(&parsed.module, "test", "test.ts").unwrap();
}

#[test]
fn just_factory() {
    lower(
        r#"
        function makeCfg() { return { scale: 1, origin: { x: 0, y: 0 } }; }
        const cfg = makeCfg();
        "#,
    );
}

#[test]
#[ignore = "pre-existing overflow in class-extends + top-level call combo; filed separately"]
fn class_extends_plus_top_level_call_overflows() {
    lower(
        r#"
        class Base { b: number; constructor(b: number) { this.b = b; } }
        class Derived extends Base {
            d: number;
            constructor(b: number, d: number) { super(b); this.d = d; }
        }
        function makeCfg() { return { scale: 1 }; }
        const cfg = makeCfg();
        "#,
    );
}
