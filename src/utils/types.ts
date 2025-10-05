export type RateMeta = {
  remaining: number;
  used: number;
  reset_at?: string;
};

export type Meta = {
  next_cursor?: string | null;
  has_more?: boolean;
  rate?: RateMeta;
};

export type ErrorShape = {
  error: { code: string; message: string; retriable: boolean };
  meta?: Meta;
};
