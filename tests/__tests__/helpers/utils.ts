import { TransactionResult } from "near-workspaces";

export function daysToMs(days: number) {
  return days * 24 * 3600 * 1000;
}

export interface ChainSignatureResponse {
  big_r: {
    affine_point: string;
  };
  s: {
    scalar: string;
  };
  recovery_id: number;
}

// whenever you need a placeholder for H256
export const someH256 =
  "00000000000000000000088feef67bf3addee2624be0da65588c032192368de8";

function parseError(e: any): string {
  try {
    const status: any =
      e && e.parse ? e.parse().result.status : JSON.parse(e.message);
    const functionCallError = status.Failure.ActionError.kind.FunctionCallError;
    return (
      functionCallError.ExecutionError ?? functionCallError.MethodResolveError
    );
  } catch {
    return e.message;
  }
}

export async function assertFailure(
  test: any,
  action: Promise<unknown>,
  errorMessage?: string,
) {
  let failed = false;

  try {
    const results = await action;
    if (results && results instanceof TransactionResult) {
      for (const outcome of results.receipts_outcomes) {
        if (outcome.isFailure) {
          failed = true;
          if (errorMessage) {
            const actualErr = JSON.stringify(outcome.executionFailure);
            test.truthy(
              JSON.stringify(actualErr).includes(errorMessage),
              `Bad error message. expected: "${errorMessage}", actual: "${actualErr}"`,
            );
          }
        }
      }
    }
  } catch (e) {
    if (errorMessage) {
      const msg: string = parseError(e);
      test.truthy(
        msg.includes(errorMessage),
        `Bad error message. expect: "${errorMessage}", actual: "${msg}"`,
      );
    }
    failed = true;
  }

  test.is(failed, true, "Function call didn't fail");
}
