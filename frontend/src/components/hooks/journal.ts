import { JournalFull } from "@/journals/api";
import { ApiError, req_api_json } from "@/net";
import { MINUTE, wait } from "@/time";
import { useQuery } from "@tanstack/react-query";
import { useMemo } from "react";
import { useParams } from "react-router-dom";

export function curr_journal_query_key(journals_id: number): ["curr_journal", number] {
    return ["curr_journal", journals_id];
}

export function useCurrJournal() {
    const { journals_id } = useParams();

    const parsed = useMemo(() => {
        if (journals_id != null) {
            let matched = /^\d+$/gm.exec(journals_id);

            if (matched != null) {
                return parseInt(journals_id, 10);
            }
        }

        return null;
    }, [journals_id]);

    const {data, isLoading, isFetching, error} = useQuery({
        queryKey: curr_journal_query_key(parsed ?? 0) as ReturnType<typeof curr_journal_query_key>,
        queryFn: async ({queryKey, client}) => {
            let [_, journals_id] = queryKey;

            try {
                return await req_api_json<JournalFull>("GET", `/journals/${journals_id}`);
            } catch (err) {
                if (err instanceof ApiError) {
                    if (err.kind === "JournalNotFound") {
                        return null;
                    }
                }

                throw err;
            }
        },
        placeholderData: (prev_data, prev_query) => prev_data,
        enabled: parsed != null,
        staleTime: ({queryKey}) => {
            const [_, journals_id] = queryKey;

            if (journals_id === 0) {
                return 0;
            } else {
                return 15 * MINUTE;
            }
        },
    });

    return {
        id: parsed,
        journal: data,
        is_loading: isLoading,
        is_fetching: isFetching,
        error
    };
}