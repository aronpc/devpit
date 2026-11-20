import type { Fields } from './stepConfig'

/*
 * Ready-made steps to start a lane's step from. Nothing is installed: a
 * recipe only fills the form, and what is saved is the person's own step.
 */

export interface Recipe {
  readonly id: 'tests' | 'review' | 'prove'
}

export const RECIPES: readonly Recipe[] = [
  { id: 'tests' },
  { id: 'review' },
  { id: 'prove' },
]

/* The answer a review must give: findings as the diff draws them, and a verdict. */
export const REVIEW_SCHEMA = JSON.stringify({
  type: 'object',
  required: ['verdict', 'findings'],
  properties: {
    verdict: { enum: ['pass', 'blocker'] },
    findings: {
      type: 'array',
      items: {
        type: 'object',
        required: ['file', 'severity', 'why'],
        properties: {
          file: { type: 'string' },
          line: { type: ['integer', 'null'], minimum: 1 },
          severity: { enum: ['blocking', 'worth', 'noted'] },
          why: { type: 'string', minLength: 1 },
        },
      },
    },
  },
})

const REVIEW_PROMPT = `Review the change this card made, and nothing before it: in the worktree path given below, the diff against the base ref given below (git diff <baseRef>). If REVIEW.md exists at the root of that checkout, review against it.

Report each problem as a finding: the file, the line on the new side, a severity — blocking (would break something if shipped), worth (worth fixing), noted (for the record) — and why. Answer verdict "blocker" when any finding is blocking, otherwise "pass".`

/* Context reaches the command only through these variables, never pasted in. */
const PROVE_COMMAND = `test_cmd='cargo test the_failing_test'  # the test that shows the bug
base=$(mktemp -d) && git worktree add -q --detach "$base" "$DEVPIT_BASE_REF" || exit 2
echo "== on the base, $DEVPIT_BASE_REF"; (cd "$base" && sh -c "$test_cmd"); before=$?
git worktree remove --force "$base"
echo "== on the card's checkout"; sh -c "$test_cmd"; after=$?
[ "$before" -ne 0 ] || { echo "not proven: the test passes on the base too"; exit 1; }
[ "$after" -eq 0 ] || { echo "not fixed: the test still fails on the checkout"; exit 1; }
echo "proven: it fails on the base and passes here"`

/* What a recipe puts in the form. `base` is config the form does not show,
   kept by the save the way an edit keeps a stored step's. */
export function filled(
  recipe: Recipe['id'],
  testCommand: string | null,
): { kind: string; name: string; fields: Fields; base: string } {
  switch (recipe) {
    case 'tests':
      return { kind: 'command', name: 'tests', fields: { command: testCommand ?? '' }, base: '{}' }
    case 'review':
      return {
        kind: 'agent',
        name: 'review',
        fields: { prompt: REVIEW_PROMPT, capUsd: '2', expects: REVIEW_SCHEMA, inject: 'card, worktreePath, baseRef' },
        base: JSON.stringify({ verdictField: 'verdict', sendsBackWhen: 'blocker' }),
      }
    case 'prove':
      return { kind: 'command', name: 'prove it', fields: { command: PROVE_COMMAND }, base: '{}' }
  }
}
