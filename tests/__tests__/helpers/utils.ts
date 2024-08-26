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
