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
