"use client";

import ky from "ky";
import { TrendingUp } from "lucide-react";
import Big from "big.js";
import {
	Bar,
	BarChart,
	CartesianGrid,
	LabelList,
	XAxis,
	YAxis,
} from "recharts";

import {
	Card,
	CardContent,
	CardDescription,
	CardFooter,
	CardHeader,
	CardTitle,
} from "@/components/ui/card";
import {
	type ChartConfig,
	ChartContainer,
	ChartTooltip,
	ChartTooltipContent,
} from "@/components/ui/chart";
import { useQuery } from "@tanstack/react-query";
import { useMemo } from "react";

const chartConfig = {
	price: {
		label: "Price",
		color: "hsl(var(--chart-2))",
	},
} satisfies ChartConfig;

const COIN_PRICES = "COIN_PRICES";
async function fetchCoinData(): Promise<{ platform: string; price: number }[]> {
	return await ky
		.get<{ platform: string; price: number }[]>("api/prices")
		.json();
}
function ArbitragePage() {
	const { data: chartData } = useQuery({
		queryKey: [COIN_PRICES],
		queryFn: fetchCoinData,
		refetchInterval: 10 * 1_000,
	});

	const highestDiff: { p1: string; p2: string; diff: number } = useMemo(() => {
		if (chartData) {
			let maxDiff = {
				p1: "",
				p2: "",
				diff: 0,
			};
			for (let i = 0; i < chartData.length; i++) {
				for (let j = 0; j < chartData.length; j++) {
					const diff = chartData[i].price - chartData[j].price;
					if (diff > maxDiff.diff) {
						maxDiff = {
							p1: chartData[i].platform,
							p2: chartData[j].platform,
							diff: new Big(diff / chartData[i].price).mul(100).toNumber(),
						};
					}
				}
			}
			return maxDiff;
		}
		return {
			p1: "",
			p2: "",
			diff: 0,
		};
	}, [chartData]);

	if (!chartData) {
		return null;
	}

	return (
		<main className="w-full flex items-center justify-center h-screen">
			<article className="size-200 items-center justify-center">
				<Card>
					<CardHeader>
						<CardTitle>
							Bar Chart - Coin Prices in Different Plarforms
						</CardTitle>
						<CardDescription>
							Real Time Data - Refetch in 10 Seconds
						</CardDescription>
					</CardHeader>
					<CardContent>
						<ChartContainer config={chartConfig}>
							<BarChart
								accessibilityLayer
								data={chartData}
								margin={{
									top: 20,
								}}
							>
								<CartesianGrid vertical={false} />
								<XAxis
									dataKey="platform"
									tickLine={false}
									tickMargin={10}
									axisLine={false}
								/>
								<YAxis
									tickFormatter={(value) => value / 1_000_000_000}
									dataKey={"price"}
									domain={[0, 10_000_000_000]}
								/>
								<ChartTooltip
									cursor={false}
									content={<ChartTooltipContent hideLabel />}
								/>
								<Bar dataKey="price" fill="var(--color-price)" radius={8}>
									<LabelList
										position="top"
										offset={12}
										className="fill-foreground"
										fontSize={12}
									/>
								</Bar>
							</BarChart>
						</ChartContainer>
					</CardContent>
					<CardFooter className="flex-col items-start gap-2 text-sm">
						<div className="flex gap-2 font-medium leading-none">
							Highest Arbitrage Possibility <TrendingUp className="h-4 w-4" />
						</div>
						<div className="leading-none text-muted-foreground">
							{highestDiff.p1} & {highestDiff.p2} = {highestDiff.diff}%
						</div>
					</CardFooter>
				</Card>
			</article>
		</main>
	);
}

export default ArbitragePage;
