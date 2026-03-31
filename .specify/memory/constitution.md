<!--
SYNC IMPACT REPORT
==================
Version change: 1.0.0 → 1.1.0
Modified principles:
  - Principle V: removed embedded YAGNI bullet (now covered by dedicated Principle VI)
Added sections:
  - Principle VI: YAGNI — You Aren't Gonna Need It (new dedicated principle)
Removed sections: N/A
Templates requiring updates:
  ✅ .specify/memory/constitution.md — this file
  ⚠ .specify/templates/plan-template.md — Constitution Check references "Principles I–V";
      must be updated to reference "Principles I–VI" and add YAGNI gate.
  ✅ .specify/templates/tasks-template.md — no further changes required.
  ✅ .specify/templates/spec-template.md — no changes required.
  ✅ .specify/templates/checklist-template.md — no changes required.
Follow-up TODOs: None — all fields resolved.

Prior report (v1.0.0, 2026-03-30):
  Version change: (unversioned template) → 1.0.0
  All Core Principles (I–V), Quality Gates, Development Workflow, Governance added.
  tasks-template.md: tests changed from OPTIONAL to MANDATORY.
  plan-template.md: Constitution Check gates enumerated.
-->

# blueprint2mod Constitution

## Core Principles

### I. Test-First Development (NON-NEGOTIABLE)

Every unit of functionality MUST begin with tests, not implementation.
The Red-Green-Refactor cycle is strictly enforced:

- Tests MUST be written before any implementation code is produced.
- Each test MUST be confirmed to fail (Red) before implementation begins — a
  test that passes without implementation is invalid.
- Implementation proceeds only to the minimum necessary to make failing tests
  pass (Green).
- Code is then refactored for clarity and maintainability without breaking
  passing tests (Refactor).
- No feature, function, or user story is considered complete without a passing
  test suite covering its acceptance scenarios.
- Skipping the Red phase (writing tests after implementation) is a constitution
  violation and MUST be flagged in code review.

**Rationale**: In a system that combines image processing, ML inference, OCR,
and 3D geometry — all with quantified accuracy targets — test-first is the only
reliable way to prevent silent regressions across the detection pipeline.

### II. Test-Driven Output Validation

Every user-facing output MUST have corresponding automated assertions that
verify correctness against the specification's success criteria.

- All success criteria (SC-001 through SC-006 and any future additions) MUST be
  encoded as runnable test assertions with reference inputs and expected results.
- The 3D export pipeline (OBJ + MTL, STL) MUST have automated geometry and
  dimension correctness tests using known reference blueprints.
- OCR, ML classification, and rule-based detection MUST each have independent
  accuracy tests measured against labeled reference datasets.
- Test coverage gaps for output correctness MUST be explicitly justified; gaps
  without justification are treated as defects.
- The end-of-processing summary report (FR-015) MUST be tested for completeness
  and accuracy across nominal and degraded-mode scenarios.

**Rationale**: The specification defines measurable accuracy targets (≥90% wall
detection, ±5% dimension tolerance, 100% SketchUp import success). These targets
are meaningless unless encoded as runnable assertions that gate every delivery.

### III. Hybrid Detection with Tested Degradation

Line tracing and architectural element classification MUST combine rule-based
heuristics with ML inference as complementary layers.

- Neither approach alone is sufficient; the hybrid pipeline is the production
  baseline.
- Rule-based-only fallback mode (activated when ML models are unavailable) MUST
  be treated as a first-class, explicitly tested scenario — not an afterthought.
- The adaptive confidence threshold system MUST have tests covering: initial
  default behavior, threshold adjustment after user corrections, and convergence
  behavior over simulated correction sequences.
- Any change to ML model selection, version, or inference path MUST be
  accompanied by updated accuracy regression tests before merging.

**Rationale**: Architectural blueprint interpretation is ambiguous by nature.
Hybrid approaches reduce error rates, but each layer must be independently
verifiable to prevent silent degradation when one layer fails or regresses.

### IV. Accuracy and Measurement Integrity

All quantified accuracy targets in the specification are non-negotiable
acceptance gates — they MUST appear as pass/fail assertions in the test suite.

- Wall detection accuracy: ≥90% on high-contrast reference blueprints (SC-002).
- Interior/exterior inference accuracy: ≥90% on reference blueprints (SC-005).
- Dimension tolerance: wall dimensions in exported models within ±5% of
  ground-truth real-world measurements (SC-004).
- SketchUp import success: 100% of successfully processed blueprints produce
  importable OBJ/STL files (SC-003).
- OCR scale discrepancy warning MUST trigger when OCR-derived scale and
  user-provided scale diverge by more than ±5%.
- Total installed footprint (binary + all downloaded ML models) MUST NOT exceed
  1 GB (SC-007); individual models MUST be ≤100 MB each.
- If any target is revised, the constitution version MUST be incremented and
  the corresponding test assertions updated before the revision takes effect.

**Rationale**: Accuracy targets derived from user needs lose their meaning if
they exist only in a specification document. Encoding them as hard test gates
ensures the system cannot ship while silently missing its commitments.

### V. Incremental Delivery

Each user story MUST be independently deliverable and testable before the next
priority begins.

- User stories are implemented in priority order (P1 → P2 → P3 → P4); work on
  a lower-priority story MUST NOT begin before the higher-priority story's tests
  pass independently.
- Session state persistence MUST remain a simple local file format — no
  networked storage, databases, or external services are permitted for session
  data.
- Complexity violations MUST be documented in the plan's Complexity Tracking
  table with justification; undocumented complexity is a constitution violation.

**Rationale**: A complex multi-pipeline desktop tool risks runaway scope and
integration debt. Incremental delivery with strict per-story test gates ensures
each capability is provably correct before the next layer is added.

### VI. YAGNI — You Aren't Gonna Need It (NON-NEGOTIABLE)

No functionality, abstraction, or generalization MAY be built unless it is
explicitly required by the current specification.

- Every line of production code MUST be traceable to a specific functional
  requirement (FR-xxx) or success criterion (SC-xxx) in the specification.
- Abstractions and helper utilities MUST NOT be created for hypothetical future
  use; three similar lines of code is preferable to a premature abstraction.
- Configurable or pluggable designs are prohibited unless the specification
  explicitly requires configurability; hard-coded values that satisfy the spec
  are always preferred over flexible frameworks that anticipate unspecified needs.
- Generic interfaces, extension points, or plugin architectures MUST NOT be
  introduced without a spec update documenting the concrete need.
- If a capability is desired but not in the specification, the correct path is
  to update the spec first via `/speckit.clarify` or `/speckit.specify` — not
  to build it speculatively.
- Any code that cannot be justified against the current spec MUST be removed,
  not commented out or hidden behind a feature flag.

**Rationale**: This project has a complex pipeline (image I/O → OCR → line
tracing → ML classification → rule-based fallback → 3D generation → export).
Speculative code in any layer compounds integration complexity across all
downstream layers. YAGNI is the primary defense against scope creep in a
multi-stage processing system where "just adding one more thing" has cascading
costs.

## Quality Gates

The following gates MUST pass before a user story is considered deliverable:

- **Red confirmed**: All new tests verified to fail before implementation starts.
- **Green achieved**: All new tests pass with the implemented code.
- **Accuracy gates**: All SC-001 through SC-006 assertions pass on reference
  inputs.
- **Degraded mode**: Rule-based-only fallback tests pass independently.
- **Export validation**: Generated OBJ/STL files pass SketchUp import smoke test.
- **Regression clean**: No previously passing tests regressed by the new change.
- **YAGNI check**: All new code is traceable to a specific FR-xxx or SC-xxx;
  no speculative abstractions or unspecified features are present.

No user story ships without all applicable gates passing.

## Development Workflow

1. Read the user story acceptance scenarios from `spec.md`.
2. Write tests that encode those scenarios — tests MUST fail at this point.
3. Confirm Red: run tests and verify failure before writing any implementation.
4. Implement the minimum code to make the tests pass (Green). No more, no less.
5. Refactor: clean up without breaking tests. Do not generalize beyond the spec.
6. Verify all quality gates (see Quality Gates section), including the YAGNI check.
7. Commit and mark the user story complete.
8. Only then begin the next priority user story.

The adaptive correction history system MUST be tested with simulated sequences
before any changes to its threshold logic are considered done.

## Governance

- This constitution supersedes all other development practices, coding standards,
  or informal conventions in this project.
- **Amendments**: Any change to a principle or quality gate requires:
  1. Documented rationale for the change.
  2. A semantic version bump (MAJOR for principle removal/redefinition, MINOR
     for additions or material expansions, PATCH for clarifications).
  3. Updated test assertions reflecting the amended target.
  4. Sync of affected templates (`tasks-template.md`, `plan-template.md`, etc.).
- **Compliance review**: Constitution Check in `plan.md` MUST enumerate all
  applicable gates from Principles I–VI and confirm each is satisfied before
  Phase 1 research proceeds.
- **Versioning policy**: MAJOR.MINOR.PATCH per the amendment rules above.
  Version appears on the line below and MUST match the Sync Impact Report.

**Version**: 1.1.1 | **Ratified**: 2026-03-30 | **Last Amended**: 2026-03-30
