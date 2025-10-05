import { z } from 'zod';

export const paginationInput = {
  cursor: z.string().optional(),
  limit: z.number().int().min(1).max(100).default(30).optional(),
};

export const restPaginationInput = {
  page: z.number().int().min(1).optional(),
  per_page: z.number().int().min(1).max(100).optional(),
};
