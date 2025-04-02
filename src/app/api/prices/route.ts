import ky from "ky";
import type { NextResponse } from "next/server";

export async function GET(request: Request) {
	const nobitex = await ky
		.get("https://api.nobitex.ir/market/stats")
		.json<{ stats: Record<`${string}-${string}`, { latest: number }> }>();
	const bitpin = await ky
		.get("https://api.bitpin.ir/api/v1/mkt/tickers/")
		.json<{ symbol: string; price: string }[]>();
	const wallex = await ky.get("https://api.wallex.ir/v1/markets").json<{
		result: {
			symbols: Record<
				string,
				{
					stats: {
						bidPrice: string;
					};
				}
			>;
		};
	}>();
	const ramzinex = await ky
		.get<{ open: number }>(
			"https://publicapi.ramzinex.com/exchange/api/v1.0/exchange/chart/statistics-24/2",
		)
		.json();

	return new Response(
		JSON.stringify([
			{
				platform: "Nobitex",
				price: nobitex.stats["btc-rls"].latest / 10,
			},
			{
				platform: "Bitpin",
				price: Number(bitpin.find((item) => item.symbol === "BTC_IRT")?.price),
			},
			{
				platform: "Wallex",
				price: Number(wallex.result.symbols.BTCTMN.stats.bidPrice),
			},
			{ platform: "Ramzinex", price: ramzinex.open / 10 },
		]),
		{
			status: 200,
		},
	);
}
