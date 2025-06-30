import { useLocation, useParams } from "react-router-dom";
import { Pencil, Trash } from "lucide-react";
import { useMutation, useQuery } from "@tanstack/react-query";
import { toast } from "sonner";

import {
    SidebarContent,
    SidebarFooter,
    SidebarGroup,
    SidebarGroupContent,
    SidebarGroupLabel,
    SidebarHeader,
    SidebarMenu,
    SidebarMenuButton,
    SidebarMenuItem,
    SidebarMenuLink,
} from "@/components/ui/sidebar";
import { ApiError, req_api_json } from "@/net";
import { Button } from "@/components/ui/button";

type SyncResult = {
    type: "Noop"
} | {
    type: "Queued",
    successful: string[]
}

function JournalSidebar() {
    const {journals_id} = useParams();
    const location = useLocation();

    if (journals_id == null) {
        throw new Error("missing journals_id param");
    }

    const {data: journal, isError, isLoading} = useQuery({
        queryKey: ["journal", journals_id] as [String, String],
        queryFn: async ({queryKey}) => {
            return await req_api_json("GET", `/journals/${queryKey[1]}`);
        }
    });

    const {mutate: sync, isPending: sync_pending} = useMutation({
        mutationKey: ["sync_journal", journals_id],
        mutationFn: async () => {
            return await req_api_json<SyncResult>("POST", `/journals/${journals_id}/sync`, {});
        },
        onSuccess: (data) => {
            if (data.type === "Noop") {
                toast("No clients to sync journal");
            } else if (data.type === "Queued") {
                toast(`Syncing journal for clients: ${data.successful.join(", ")}`);
            }
        },
        onError: (err) => {
            if (err instanceof ApiError) {
                toast(`Failed to sync journal: ${err.kind}`);
            } else {
                console.error("failed to sync journal", err);

                toast(`Failed to sync journal`);
            }
        }
    });

    let journal_name = "Journal";

    if (isLoading) {
        journal_name = "Loading...";
    } else if (!isError) {
        journal_name = journal.name;
    }

    return <>
        <SidebarHeader className="border-b">
            <h2 className="w-full text-xl overflow-hidden text-nowrap whitespace-nowrap text-ellipsis" title={journal_name}>
                {journal_name}
            </h2>
        </SidebarHeader>
        <SidebarContent>
            <SidebarGroup>
                <SidebarGroupLabel>Entries</SidebarGroupLabel>
                <SidebarGroupContent>
                    <SidebarMenu>
                        <SidebarMenuLink
                            title="Search"
                            tooltip={"Search all available entries for the journal."}
                            path={`/journals/${journals_id}/entries`}
                            active={location.pathname.startsWith(`/journals/${journals_id}/entries`)}
                        />
                    </SidebarMenu>
                </SidebarGroupContent>
            </SidebarGroup>
            <SidebarGroup>
                <SidebarGroupLabel>
                    Actions
                </SidebarGroupLabel>
                <SidebarGroupContent>
                    <SidebarMenu>
                        <SidebarMenuLink
                            title="Edit"
                            tooltip="Edit the current journal"
                            path={`/journals/${journals_id}`}
                            active={location.pathname === `/journals/${journals_id}`}
                            icon={<Pencil/>}
                        />
                        <SidebarMenuItem>
                            <SidebarMenuButton
                                disabled={sync_pending}
                                tooltip={{children: "Synchronize journal with attached peers.", hidden: false}}
                                onClick={() => sync()}
                            >
                                Synchronize
                            </SidebarMenuButton>
                        </SidebarMenuItem>
                    </SidebarMenu>
                </SidebarGroupContent>
            </SidebarGroup>
        </SidebarContent>
        <SidebarFooter>
            <Button type="button" variant="destructive">
                <Trash/>Delete
            </Button>
        </SidebarFooter>
    </>
}

export { JournalSidebar };
