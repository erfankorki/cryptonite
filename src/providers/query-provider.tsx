"use client";

import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { useState, type ReactNode } from "react";

export default function QueryProvider({ children }: { children: ReactNode }) {
	const [queryClient] = useState(
		() =>
			new QueryClient({
				defaultOptions: {
					queries: {
						staleTime: 0,
						gcTime: 0,
						retry: false,
					},
				},
			}),
	);
	return (
		<QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
	);
}
