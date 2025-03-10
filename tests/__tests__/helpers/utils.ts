import { NEAR, TransactionResult } from "near-workspaces";
import * as borsh from "borsh";

const STORAGE_PRICE_PER_BYTE = 10_000_000_000_000_000_000n;

export function daysToMs(days: number) {
  return days * 24 * 3600 * 1000;
}

export function getStorageDeposit(bytes: number) {
  return NEAR.from(STORAGE_PRICE_PER_BYTE.toString()).muln(bytes);
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

export function buildDepositEmbedMsg(
  depositVout: number,
  pubkey: string,
  sequence: number,
) {
  const schema = {
    enum: [
      {
        struct: {
          V1: {
            struct: {
              deposit_vout: "u64",
              user_pubkey: { array: { type: "u8", len: 33 } },
              sequence_height: "u16",
            },
          },
        },
      },
    ],
  };
  const data = {
    V1: {
      deposit_vout: depositVout,
      user_pubkey: Buffer.from(pubkey, "hex"),
      sequence_height: sequence,
    },
  };
  const msg = borsh.serialize(schema, data);
  const magicHeader = "bithive";
  return Buffer.concat([Buffer.from(magicHeader), msg]);
}
