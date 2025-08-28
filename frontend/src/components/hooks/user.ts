import { req_api_json } from "@/net";
import { useQuery } from "@tanstack/react-query";

export function curr_user_query_key(): ["curr_user"] {
    return ["curr_user"];
}

export interface CurrUser {
    id: number,
    username: string,
}

export function useCurrUser() {
    const {data, isLoading, error} = useQuery<CurrUser, Error, CurrUser, ReturnType<typeof curr_user_query_key>>({
        queryKey: curr_user_query_key(),
        queryFn: async () => {
            return await req_api_json<CurrUser>("GET", "/me");
        },
        // force the data to never be stale so when it is set during
        // login / verify it will not fetch the data again
        staleTime: Infinity,
    });

    return {
        // if the placeholder is available then this should always have a value
        // but for some reason typescript is saying that it can still be
        // undefined.
        user: data ?? {
            id: 0,
            username: "Unknown"
        },
        is_loading: isLoading,
        error
    };
}