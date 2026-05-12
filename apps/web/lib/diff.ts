export type DiffOp =
  | { type: "equal"; text: string }
  | { type: "delete"; text: string }
  | { type: "insert"; text: string };

export function wordDiff(original: string, proposed: string): DiffOp[] {
  // Split on whitespace, keeping the separators as tokens
  const origTokens = original.split(/(\s+)/);
  const propTokens = proposed.split(/(\s+)/);
  const n = origTokens.length;
  const m = propTokens.length;

  // LCS table (backward DP)
  const dp: number[][] = Array.from({ length: n + 1 }, () =>
    new Array(m + 1).fill(0),
  );
  for (let i = n - 1; i >= 0; i--) {
    for (let j = m - 1; j >= 0; j--) {
      dp[i][j] =
        origTokens[i] === propTokens[j]
          ? dp[i + 1][j + 1] + 1
          : Math.max(dp[i + 1][j], dp[i][j + 1]);
    }
  }

  const result: DiffOp[] = [];
  let i = 0;
  let j = 0;
  while (i < n || j < m) {
    if (i < n && j < m && origTokens[i] === propTokens[j]) {
      result.push({ type: "equal", text: origTokens[i] });
      i++;
      j++;
    } else if (j < m && (i >= n || dp[i + 1][j] <= dp[i][j + 1])) {
      result.push({ type: "insert", text: propTokens[j] });
      j++;
    } else {
      result.push({ type: "delete", text: origTokens[i] });
      i++;
    }
  }

  // Merge adjacent same-type ops so whitespace tokens coalesce
  const merged: DiffOp[] = [];
  for (const op of result) {
    const prev = merged[merged.length - 1];
    if (prev && prev.type === op.type) {
      prev.text += op.text;
    } else {
      merged.push({ ...op });
    }
  }
  return merged;
}
