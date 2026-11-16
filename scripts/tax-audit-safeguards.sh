#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

# shellcheck source=scripts/step-runner.sh
source "$ROOT_DIR/scripts/step-runner.sh"

# Tax-audit runtime and setup safeguards against an already built
# ./target/release/runa; this script never builds. CI runs it with
# --keep-going so one run reports every failing safeguard.
steps_init "[safeguards]" "tax-audit" "$@"

RELEASE_RUNA="./target/release/runa"
MODEL_RUNA="$ROOT_DIR/target/release/runa"

prerequisite test -x "$RELEASE_RUNA"
step tax-audit env RUNA_BIN="$MODEL_RUNA" bash scripts/tax-audit-preflight.sh
step tax-audit env FUTURUNA_MODEL_TEST_RUNA="$MODEL_RUNA" python3 -m unittest discover -s scripts/tests -p 'test_calculation_input_guide.py'
step tax-audit "$RELEASE_RUNA" tests/personskat_recurring_gifts_test.runa
step tax-audit "$RELEASE_RUNA" run tests/personskat_recurring_gifts_test.runa
step tax-audit env FUTURUNA_MODEL_TEST_RUNA="$MODEL_RUNA" node --test tests/tax_audit_setup.test.mjs tests/pension_demo_input.test.mjs tests/tax_report_viewer.test.mjs tests/personskat_viewer.test.mjs tests/tax_municipality.test.mjs tests/green_check.test.mjs tests/personskat_green_check.test.mjs
step tax-audit env FUTURUNA_MODEL_TEST_RUNA="$MODEL_RUNA" node --test tests/personskat_recurring_gifts.test.mjs
step tax-audit "$RELEASE_RUNA" tests/personskat_maintenance_recipient_test.runa
step tax-audit env FUTURUNA_MODEL_TEST_RUNA="$MODEL_RUNA" node --test tests/personskat_maintenance_recipient.test.mjs
step tax-audit "$RELEASE_RUNA" tests/personskat_ferry_test.runa
step tax-audit "$RELEASE_RUNA" run tests/personskat_ferry_test.runa
step tax-audit env FUTURUNA_MODEL_TEST_RUNA="$MODEL_RUNA" node --test --test-concurrency=1 tests/personskat_ferry.test.mjs tests/personskat_commuting_days.test.mjs
step tax-audit env FUTURUNA_MODEL_TEST_RUNA="$MODEL_RUNA" node --test tests/tax_salary_metadata.test.mjs
step tax-audit "$RELEASE_RUNA" tests/personskat_salary_mapping_test.runa
step tax-audit env FUTURUNA_MODEL_TEST_RUNA="$MODEL_RUNA" node --test tests/tax_salary_mapping_metadata.test.mjs
step tax-audit "$RELEASE_RUNA" tests/personskat_interest_mapping_test.runa
step tax-audit env FUTURUNA_MODEL_TEST_RUNA="$MODEL_RUNA" node --test tests/tax_interest_mapping_metadata.test.mjs
step tax-audit env FUTURUNA_MODEL_TEST_RUNA="$MODEL_RUNA" node --test tests/personskat_salary_input.test.mjs
step tax-audit "$RELEASE_RUNA" tests/personskat_partyear_input_controls_test.runa
step tax-audit env FUTURUNA_MODEL_TEST_RUNA="$MODEL_RUNA" node --test --test-concurrency=1 tests/personskat_settlement_stage.test.mjs
step tax-audit env FUTURUNA_MODEL_TEST_RUNA="$MODEL_RUNA" node --test --test-concurrency=1 tests/tax_credit_input.test.mjs tests/tax_credit_signs.test.mjs
step tax-audit "$RELEASE_RUNA" tests/personskat_refund_mapping_test.runa
step tax-audit env FUTURUNA_MODEL_TEST_RUNA="$MODEL_RUNA" node --test tests/tax_refund_mapping_metadata.test.mjs
step tax-audit env FUTURUNA_MODEL_TEST_RUNA="$MODEL_RUNA" node --test tests/tax_supplementary_metadata.test.mjs
step tax-audit env FUTURUNA_MODEL_TEST_RUNA="$MODEL_RUNA" node --test --test-concurrency=1 tests/tax_report_input_metadata.test.mjs tests/tax_report_payment.test.mjs
step tax-audit env FUTURUNA_MODEL_TEST_RUNA="$MODEL_RUNA" node --test tests/tax_church_input.test.mjs
step tax-audit env FUTURUNA_MODEL_TEST_RUNA="$MODEL_RUNA" node --test tests/tax_spouse_input.test.mjs
step tax-audit env FUTURUNA_MODEL_TEST_RUNA="$MODEL_RUNA" node --test tests/tax_interest_input.test.mjs
step tax-audit "$RELEASE_RUNA" tests/personskat_bundskat_spouse_test.runa
step tax-audit env FUTURUNA_MODEL_TEST_RUNA="$MODEL_RUNA" node --test tests/personskat_married_interest.test.mjs
step tax-audit env FUTURUNA_MODEL_TEST_RUNA="$MODEL_RUNA" node --test tests/tax_source_input.test.mjs
step tax-audit env FUTURUNA_MODEL_TEST_RUNA="$MODEL_RUNA" node --test tests/pension_couple_demo.test.mjs
step tax-audit env FUTURUNA_MODEL_TEST_RUNA="$MODEL_RUNA" node --test tests/personskat_partyear_validity.test.mjs
step tax-audit env FUTURUNA_MODEL_TEST_RUNA="$MODEL_RUNA" node --test tests/personskat_partyear_birth_dates.test.mjs
step tax-audit env FUTURUNA_MODEL_TEST_RUNA="$MODEL_RUNA" node --test tests/personskat_partyear_spouse_settings.test.mjs
step tax-audit env FUTURUNA_MODEL_TEST_RUNA="$MODEL_RUNA" node --test tests/personskat_partyear_capital.test.mjs
step tax-audit env FUTURUNA_MODEL_TEST_RUNA="$MODEL_RUNA" node --test tests/personskat_partyear_shares.test.mjs
step tax-audit env FUTURUNA_MODEL_TEST_RUNA="$MODEL_RUNA" node --test tests/personskat_partyear_share_conversion.test.mjs
step tax-audit "$RELEASE_RUNA" tests/personskat_work_income_route_test.runa
step tax-audit env FUTURUNA_MODEL_TEST_RUNA="$MODEL_RUNA" node --test tests/personskat_work_income_route.test.mjs
step tax-audit env FUTURUNA_MODEL_TEST_RUNA="$MODEL_RUNA" node --test tests/personskat_property_route.test.mjs
step tax-audit "$RELEASE_RUNA" tests/personskat_property_age_test.runa
step tax-audit env FUTURUNA_MODEL_TEST_RUNA="$MODEL_RUNA" node --test tests/personskat_property_age.test.mjs
step tax-audit env FUTURUNA_MODEL_TEST_RUNA="$MODEL_RUNA" node --test tests/personskat_financial_route.test.mjs
step tax-audit "$RELEASE_RUNA" tests/personskat_honorar_expenses_test.runa
step tax-audit env FUTURUNA_MODEL_TEST_RUNA="$MODEL_RUNA" node --test tests/personskat_honorar_expenses.test.mjs
step tax-audit env FUTURUNA_MODEL_TEST_RUNA="$MODEL_RUNA" node --test tests/tax_employer_pension_conformance.test.mjs
step tax-audit "$RELEASE_RUNA" tests/personskat_pension_payment_year_test.runa
step tax-audit env FUTURUNA_MODEL_TEST_RUNA="$MODEL_RUNA" node --test tests/personskat_pension_payment_year.test.mjs
step tax-audit "$RELEASE_RUNA" tests/personskat_pension_redeposit_test.runa
step tax-audit env FUTURUNA_MODEL_TEST_RUNA="$MODEL_RUNA" node --test tests/personskat_pension_redeposit.test.mjs
step tax-audit "$RELEASE_RUNA" tests/personskat_pension_bank_year_test.runa
step tax-audit env FUTURUNA_MODEL_TEST_RUNA="$MODEL_RUNA" node --test tests/personskat_pension_bank_year.test.mjs
step tax-audit env FUTURUNA_MODEL_TEST_RUNA="$MODEL_RUNA" node --test tests/personskat_group_life.test.mjs
step tax-audit "$RELEASE_RUNA" tests/personskat_group_bonus_test.runa
step tax-audit "$RELEASE_RUNA" run tests/personskat_group_bonus_test.runa
step tax-audit env FUTURUNA_MODEL_TEST_RUNA="$MODEL_RUNA" node --test tests/personskat_group_bonus.test.mjs
step tax-audit "$RELEASE_RUNA" tests/personskat_employee_expenses_test.runa
step tax-audit "$RELEASE_RUNA" run tests/personskat_employee_expenses_test.runa
step tax-audit env FUTURUNA_MODEL_TEST_RUNA="$MODEL_RUNA" node --test tests/personskat_employee_expenses.test.mjs
step tax-audit "$RELEASE_RUNA" tests/personskat_boligjob_rules_test.runa
step tax-audit "$RELEASE_RUNA" run tests/personskat_boligjob_rules_test.runa
step tax-audit env FUTURUNA_MODEL_TEST_RUNA="$MODEL_RUNA" node --test tests/boligjob_supplier_age.test.mjs
step tax-audit "$RELEASE_RUNA" tests/personskat_labour_hire_age_test.runa
step tax-audit "$RELEASE_RUNA" run tests/personskat_labour_hire_age_test.runa
step tax-audit env FUTURUNA_MODEL_TEST_RUNA="$MODEL_RUNA" node --test tests/personskat_labour_hire_age.test.mjs
step tax-audit env FUTURUNA_MODEL_TEST_RUNA="$MODEL_RUNA" node --test tests/personskat_dis_coverage.test.mjs

steps_finish
echo
echo "[safeguards] All tax-audit safeguards passed."
