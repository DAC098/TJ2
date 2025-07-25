import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { format, formatDistanceToNow } from "date-fns";
import { ArrowLeft, CalendarIcon, ChevronDown, ChevronUp, CircleCheck, CircleDashed, CircleX, Copy, Plus, RefreshCw, Save, Trash } from "lucide-react";
import { useEffect, useState } from "react";
import { FormProvider, SubmitHandler, useForm, useFormContext } from "react-hook-form";
import { Link, useNavigate, useParams } from "react-router-dom";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import { CenterPage } from "@/components/ui/page";
import { ColumnDef, DataTable } from "@/components/ui/table";
import { ApiError, req_api_json, req_api_json_empty } from "@/net";
import { FormControl, FormDescription, FormField, FormItem, FormLabel, FormMessage, FormRootError } from "@/components/ui/form";
import { Input } from "@/components/ui/input";
import { Checkbox } from "@/components/ui/checkbox";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import { Separator } from "@/components/ui/separator";
import { send_to_clipboard } from "@/utils";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle, DialogTrigger } from "@/components/ui/dialog";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { Calendar } from "@/components/ui/calendar";

interface JournalSharePartial {
    id: number,
    name: string,
    created: string,
    updated: string,
    users: number,
    pending_invites: number
}

interface JournalShareFull {
    id: number,
    name: string,
    created: string,
    updated: string | null,
    abilities: JournalShareAbility[],
}

type JournalShareAbility = "JournalUpdate" | "EntryCreate" | "EntryUpdate" | "EntryDelete";

function search_query_key(journals_id: string): ["journal_shares", string] {
    return ["journal_shares", journals_id];
}

function edit_query_key(journals_id: string, share_id: string): ["journal_share_edit", string, string] {
    return ["journal_share_edit", journals_id, share_id];
}

function search_users_query_key(journals_id: string, share_id: string): ["journal_share_users", {journals_id: string, share_id: string}] {
    return ["journal_share_users", {journals_id, share_id}];
}

function search_invites_query_key(journals_id: string, share_id: string): ["journal_share_invites", {journals_id: string, share_id: string}] {
    return ["journal_share_invites", {journals_id, share_id}];
}

function blank_journal_share(): JournalShareFull {
    return {
        id: 0,
        name: "",
        created: (new Date()).toJSON(),
        updated: null,
        abilities: [],
    };
}

export function JournalShareSearch() {
    const { journals_id } = useParams();

    if (journals_id == null) {
        throw new Error("journals_id is not provided");
    }

    let {data, error, isLoading, refetch} = useQuery({
        queryKey: search_query_key(journals_id),
        queryFn: async ({queryKey}) => {
            return await req_api_json<JournalSharePartial[]>("GET", `/journals/${queryKey[1]}/share`);
        }
    });

    const columns: ColumnDef<JournalSharePartial>[] = [
        {
            header: "Name",
            cell: ({ row }) => {
                return <Link to={`/journals/${journals_id}/share/${row.original.id}`}>{row.original.name}</Link>;
            }
        },
        {
            header: "Users",
            accessorKey: "users",
        },
        {
            header: "Pending Invites",
            accessorKey: "pending_invites"
        },
        {
            header: "Mod",
            cell: ({ row }) => {
                let to_use = row.original.updated != null ?
                    new Date(row.original.updated) :
                    new Date(row.original.created);
                let distance = formatDistanceToNow(to_use, {
                    addSuffix: true,
                    includeSeconds: true,
                });

                return <span title={to_use.toString()} className="text-nowrap">{distance}</span>;
            }
        }
    ];

    return <CenterPage className="pt-4 max-w-6xl">
        <div className="flex flex-row flex-nowrap gap-x-4">
            <Button type="button" variant="secondary" size="icon" disabled={isLoading} onClick={() => refetch()}>
                <RefreshCw/>
            </Button>
            <Link to={`/journals/${journals_id}/share/new`}>
                <Button type="button">
                    <Plus/>New Share
                </Button>
            </Link>
        </div>
        <DataTable columns={columns} data={data ?? []} empty={isLoading ? "Loading..." : "No Journal Shares"}/>
    </CenterPage>;
}

export function JournalShareEdit() {
    const { journals_id, share_id } = useParams();

    if (journals_id == null || share_id == null) {
        throw new Error("missing journals_id or share_id");
    }

    const [view_users, set_view_users] = useState(false);
    const [view_invites, set_view_invites] = useState(false);

    const {data, isLoading} = useQuery<JournalShareFull>({
        queryKey: edit_query_key(journals_id, share_id),
        queryFn: async ({queryKey}) => {
            if (queryKey[2] === "new") {
                return blank_journal_share();
            }

            return await req_api_json("GET", `/journals/${queryKey[1]}/share/${queryKey[2]}`);
        }
    });

    if (isLoading) {
        return <div>Loading...</div>;
    }

    if (data == null) {
        return <div>Failed to load journal share information</div>;
    }

    return <CenterPage>
        <JournalShareCoreEdit journals_id={journals_id} share_id={share_id} journal_share={data}/>
        <Separator/>
        <Collapsible open={view_invites} onOpenChange={open => {
            set_view_invites(open);
        }}>
            <div className="flex flex-row items-center">
                <h3 className="text-lg">Invites</h3>
                <div className="flex-1"/>
                <CollapsibleTrigger asChild>
                    <Button variant="ghost" size="icon" disabled={share_id === "new"}>
                        {view_invites ? <ChevronUp/> : <ChevronDown/>}
                    </Button>
                </CollapsibleTrigger>
            </div>
            <CollapsibleContent>
                <InvitesView journals_id={journals_id} share_id={share_id}/>
            </CollapsibleContent>
        </Collapsible>
        <Separator/>
        <Collapsible open={view_users} onOpenChange={open => {
            set_view_users(open);
        }}>
            <div className="flex flex-row items-center">
                <h3 className="text-lg">Users</h3>
                <div className="flex-1"/>
                <CollapsibleTrigger asChild>
                    <Button variant="ghost" size="icon" disabled={share_id === "new"}>
                        {view_users ? <ChevronUp/> : <ChevronDown/>}
                    </Button>
                </CollapsibleTrigger>
            </div>
            <CollapsibleContent>
                <AttachedUserView journals_id={journals_id} share_id={share_id}/>
            </CollapsibleContent>
        </Collapsible>
    </CenterPage>;
}

interface JournalShareForm {
    name: string,
    abilities: {
        "JournalRead": boolean,
        "JournalUpdate": boolean,
        "EntryRead": boolean,
        "EntryCreate": boolean,
        "EntryUpdate": boolean,
        "EntryDelete": boolean,
    }
}

function get_form_state(given: JournalShareFull): JournalShareForm {
    let rtn = {
        name: given.name.slice(0),
        abilities: {
            "JournalRead": false,
            "JournalUpdate": false,
            "EntryRead": false,
            "EntryCreate": false,
            "EntryUpdate": false,
            "EntryDelete": false,
        },
    };

    for (let ability of given.abilities) {
        if (ability in rtn.abilities) {
            rtn.abilities[ability] = true;
        }
    }

    return rtn;
}

interface JournalShareCoreEditHeaderProps {
    journals_id: string,
    share_id: string,
}

function JournalShareCoreEditHeader({journals_id, share_id}: JournalShareCoreEditHeaderProps) {
    const navigate = useNavigate();
    const form = useFormContext<JournalShareForm>();

    const {mutate: delete_share, isPending} = useMutation({
        mutationFn: async () => {
            await req_api_json_empty("DELETE", `/journals/${journals_id}/share/${share_id}`);
        },
        onSuccess: (data, vars, ctx) => {
            toast("Deleted Journal Share");

            navigate(`/journals/${journals_id}/share`);
        },
        onError: (err, vars, ctx) => {
            if (err instanceof ApiError) {
                toast(`Failed to delete journal share: ${err.kind}`);
            } else {
                console.error("failed to delete journal share:", err);

                toast("Failed to delete journal share: ClientError");
            }
        }
    });

    return <div className="top-0 sticky flex flex-row flex-nowrap gap-x-4 bg-background border-b py-2">
        <Link to={`/journals/${journals_id}/share`}>
            <Button type="button" variant="ghost" size="icon">
                <ArrowLeft/>
            </Button>
        </Link>
        <FormField control={form.control} name="name" render={({field}) => {
            return <FormItem>
                <FormControl>
                    <Input type="text" placeholder="name" {...field}/>
                </FormControl>
                <FormMessage/>
            </FormItem>
        }}/>
        <Button type="submit" disabled={!form.formState.isDirty || isPending}>
            Save<Save/>
        </Button>
        {share_id !== "new" ?
            <Button
                type="button"
                variant="destructive"
                disabled={form.formState.isSubmitting || isPending}
                onClick={() => delete_share()}
            >
                Delete<Trash/>
            </Button>
            :
            null
        }
    </div>;
}

interface JournalShareCoreEditProps {
    journals_id: string,
    share_id: string,
    journal_share: JournalShareFull,
}

function JournalShareCoreEdit({journals_id, share_id, journal_share}: JournalShareCoreEditProps) {
    const client = useQueryClient();
    const navigate = useNavigate();

    const form = useForm<JournalShareForm>({
        defaultValues: get_form_state(journal_share),
    });

    useEffect(() => {
        form.reset(get_form_state(journal_share));
    }, [share_id]);

    const on_submit: SubmitHandler<JournalShareForm> = async (data, event) => {
        try {
            let body: any = {
                name: data.name,
                abilities: []
            };

            for (let key in data.abilities) {
                if (data.abilities[key as JournalShareAbility]) {
                    body.abilities.push(key);
                }
            }

            if (share_id === "new") {
                let res = await req_api_json<JournalShareFull>("POST", `/journals${journals_id}/share`, body);

                client.setQueryData(edit_query_key(journals_id, res.id.toString(10)), res);

                toast("Create New Journal Share");

                navigate(`/journals/${journals_id}/share/${res.id}`);
            } else {
                let res = await req_api_json<JournalShareFull>("PATCH", `/journals/${journals_id}/share/${share_id}`, body);

                client.setQueryData(edit_query_key(journals_id, res.id.toString(10)), res);

                toast("Updated Journal Share");

                form.reset(get_form_state(res));
            }
        } catch (err) {
            if (err instanceof ApiError) {
                switch (err.kind) {
                    case "NameAlreadyExists":
                        form.setError("name", {
                            message: "This name already exists",
                        });
                        break;
                    case "ShareNotFound":
                        form.setError("root", {
                            message: "The requested share was not found.",
                        });
                        break;
                    case "JournalNotFound":
                        form.setError("root", {
                            message: "the requested journal was not found.",
                        });
                        break;
                    default:
                        form.setError("root", {
                            message: `Failed to update journal share: ${err.kind}`,
                        });
                        break;
                }
            } else {
                console.error("failed to update journal share:", err);

                form.setError("root", {
                    message: "Failed to update journal share: ClientError",
                });
            }
        }
    };

    return <FormProvider<JournalShareForm> {...form} children={
        <form onSubmit={form.handleSubmit(on_submit)}>
            <JournalShareCoreEditHeader journals_id={journals_id} share_id={share_id}/>
            <div className="space-y-4 pt-2">
                <FormRootError/>
                <h2 className="text-xl font-medium">Journal</h2>
                <div className="grid grid-cols-2 gap-2">
                    <FormField control={form.control} name="abilities.JournalRead" render={({field}) => {
                        return <FormItem className="flex flex-row items-start space-x-3 space-y-0 rounded-md border p-4">
                            <FormControl>
                                <Checkbox checked={field.value} onCheckedChange={field.onChange}/>
                            </FormControl>
                            <div className="space-y-1 leading-none">
                                <FormLabel>Journal Read</FormLabel>
                                <FormDescription>
                                    Allows for the ability to read information about the journal.
                                </FormDescription>
                            </div>
                        </FormItem>;
                    }}/>
                    <FormField control={form.control} name="abilities.JournalUpdate" render={({field}) => {
                        return <FormItem className="flex flex-row items-start space-x-3 space-y-0 rounded-md border p-4">
                            <FormControl>
                                <Checkbox checked={field.value} onCheckedChange={field.onChange}/>
                            </FormControl>
                            <div className="space-y-1 leading-none">
                                <FormLabel>Journal Update</FormLabel>
                                <FormDescription>
                                    Allows for the ability to update certain parts of the journal.
                                </FormDescription>
                            </div>
                        </FormItem>;
                    }}/>
                </div>
                <h2 className="text-xl font-medium">Entries</h2>
                <div className="grid grid-cols-2 gap-2">
                    <FormField control={form.control} name="abilities.EntryRead" render={({field}) => {
                        return <FormItem className="flex flex-row items-start space-x-3 space-y-0 rounded-md border p-4">
                            <FormControl>
                                <Checkbox checked={field.value} onCheckedChange={field.onChange}/>
                            </FormControl>
                            <div className="space-y-1 leading-none">
                                <FormLabel>Entry Read</FormLabel>
                                <FormDescription>
                                    Allows for the ability to read entries for the journal.
                                </FormDescription>
                            </div>
                        </FormItem>;
                    }}/>
                    <FormField control={form.control} name="abilities.EntryCreate" render={({field}) => {
                        return <FormItem className="flex flex-row items-start space-x-3 space-y-0 rounded-md border p-4">
                            <FormControl>
                                <Checkbox checked={field.value} onCheckedChange={field.onChange}/>
                            </FormControl>
                            <div className="space-y-1 leading-none">
                                <FormLabel>Entry Create</FormLabel>
                                <FormDescription>
                                    Allows for the ability to create new entries for the journal.
                                </FormDescription>
                            </div>
                        </FormItem>;
                    }}/>
                    <FormField control={form.control} name="abilities.EntryUpdate" render={({field}) => {
                        return <FormItem className="flex flex-row items-start space-x-3 space-y-0 rounded-md border p-4">
                            <FormControl>
                                <Checkbox checked={field.value} onCheckedChange={field.onChange}/>
                            </FormControl>
                            <div className="space-y-1 leading-none">
                                <FormLabel>Entry Update</FormLabel>
                                <FormDescription>
                                    Allows for the ability to update existing entries for read journal.
                                </FormDescription>
                            </div>
                        </FormItem>;
                    }}/>
                    <FormField control={form.control} name="abilities.EntryDelete" render={({field}) => {
                        return <FormItem className="flex flex-row items-start space-x-3 space-y-0 rounded-md border p-4">
                            <FormControl>
                                <Checkbox checked={field.value} onCheckedChange={field.onChange}/>
                            </FormControl>
                            <div className="space-y-1 leading-none">
                                <FormLabel>Entry Delete</FormLabel>
                                <FormDescription>
                                    Allows for the ability to delete existing entries for read journal.
                                </FormDescription>
                            </div>
                        </FormItem>;
                    }}/>
                </div>
            </div>
        </form>
    }/>;
}

interface AttachedUser {
    id: number,
    username: string,
    added: string,
}

interface AttachedUserViewProps {
    journals_id: string,
    share_id: string
}

function AttachedUserView({journals_id, share_id}: AttachedUserViewProps) {
    const {data, isLoading, isError, refetch} = useQuery<AttachedUser[], Error, AttachedUser[], ReturnType<typeof search_users_query_key>>({
        queryKey: search_users_query_key(journals_id, share_id),
        queryFn: async ({queryKey}) => {
            return await req_api_json("GET", `/journals/${queryKey[1].journals_id}/share/${queryKey[1].share_id}/users`);
        }
    });

    const {mutate: remove_user} = useMutation<void, Error, {users_id: number}>({
        mutationFn: async ({users_id}) => {
            await req_api_json_empty("DELETE", `/journals/${journals_id}/share/${share_id}/users`, {
                type: "Single",
                users_id
            });
        },
        onSuccess: (data, {users_id}, ctx) => {
            refetch();
        },
        onError: (err, vars, ctx) => {
            if (err instanceof ApiError) {
                toast(`Failed to remove user: ${err.kind}`);
            } else {
                console.error("failed to remove user:", err);

                toast(`Failed to remove user: ClientError`);
            }
        }
    });

    const columns: ColumnDef<AttachedUser>[] = [
        {
            header: "Username",
            accessorKey: "username"
        },
        {
            header: "Added",
            cell: ({ row }) => {
                let to_use = new Date(row.original.added);
                let distance = formatDistanceToNow(to_use, {
                    addSuffix: true,
                    includeSeconds: true,
                });

                return <span title={to_use.toString()} className="text-nowrap">{distance}</span>;
            }
        },
        {
            id: "action",
            cell: ({ row }) => {
                return <Button type="button" variant="destructive" size="icon" onClick={() => {
                    remove_user({users_id: row.original.id});
                }}>
                    <Trash/>
                </Button>;
            }
        }
    ];

    return <div className="space-y-2">
        <div className="flex flex-row flex-nowrap gap-x-4">
            <Button type="button" variant="secondary" size="icon" disabled={isLoading} onClick={() => refetch()}>
                <RefreshCw />
            </Button>
        </div>
        <DataTable columns={columns} data={data ?? []} empty={isLoading ? "Loading..." : "No Users Attached"}/>
    </div>
}

interface InviteFull {
    token: string,
    user: InviteUser | null,
    issued_on: string,
    expires_on: string | null
    status: InviteStatus,
}

enum InviteStatus {
    Pending = 0,
    Accepted = 1,
    Rejected = 2,
}

interface InviteUser {
    id: number,
    username: string
}

interface InvitesViewProps {
    journals_id: string,
    share_id: string
}

function InvitesView({journals_id, share_id}: InvitesViewProps) {
    const {data, isLoading, isError, refetch} = useQuery<InviteFull[], Error, InviteFull[], ReturnType<typeof search_invites_query_key>>({
        queryKey: search_invites_query_key(journals_id, share_id),
        queryFn: async ({queryKey}) => {
            return await req_api_json("GET", `/journals/${queryKey[1].journals_id}/share/${queryKey[1].share_id}/invite`);
        }
    });

    const {mutate: delete_invite} = useMutation<void, Error, {token: string}>({
        mutationFn: async ({token}) => {
            await req_api_json_empty("DELETE", `/journals/${journals_id}/share/${share_id}/invite`, {
                type: "Single",
                token
            });
        },
        onSuccess: (data, {token}, ctx) => {
            refetch();
        },
        onError: (err, vars, ctx) => {
            if (err instanceof ApiError) {
                toast(`Failed to delete invite: ${err.kind}`);
            } else {
                console.error("failed to delete invite", err);

                toast(`Failed to delete invite: ClientError`);
            }
        }
    });

    const columns: ColumnDef<InviteFull>[] = [
        {
            header: "Token",
            cell: ({ row }) => {
                return <Button type="button" variant="ghost" size="sm" onClick={() => {
                    send_to_clipboard(row.original.token).then(() => {
                        toast("Copied to clipboard");
                    }).catch(err => {
                        console.error(err);
                        toast(`Failed copying to clipboard: ${row.original.token}`);
                    });
                }}>
                    {row.original.token}
                    <Copy/>
                </Button>
            }
        },
        {
            header: "Status",
            cell: ({ row }) => {
                switch (row.original.status) {
                    case InviteStatus.Pending:
                        return <div className="flex flex-row items-center gap-2">
                            <CircleDashed size={18}/> Pending
                        </div>;
                    case InviteStatus.Accepted:
                        return <div className="flex flex-row items-center gap-2">
                            <CircleCheck size={18} className="text-green-500"/> Accepted
                        </div>;
                    case InviteStatus.Rejected:
                        return <div className="flex flex-row items-center gap-2">
                            <CircleX size={18} className="text-red-500"/> Rejected
                        </div>;
                }
            }
        },
        {
            header: "User",
            cell: ({ row }) => {
                if (row.original.user != null) {
                    return row.original.user.username;
                } else {
                    return null;
                }
            }
        },
        {
            header: "Issued On",
            cell: ({ row }) => {
                let to_use = new Date(row.original.issued_on);
                let distance = formatDistanceToNow(to_use, {
                    addSuffix: true,
                    includeSeconds: true,
                });

                return <span title={to_use.toString()} className="text-nowrap">{distance}</span>;
            }
        },
        {
            header: "Expires On",
            cell: ({ row }) => {
                if (row.original.expires_on != null) {
                    let to_use = new Date(row.original.expires_on);
                    let distance = formatDistanceToNow(to_use, {
                        addSuffix: true,
                        includeSeconds: true,
                    });

                    return <span title={to_use.toString()} className="text-nowrap">{distance}</span>;
                } else {
                    return null;
                }
            }
        },
        {
            id: "actions",
            cell: ({ row }) => {
                return <Button type="button" variant="destructive" size="icon" onClick={() => {
                    delete_invite({token: row.original.token});
                }}>
                    <Trash/>
                </Button>
            }
        }
    ];

    return <div className="space-y-2">
        <div className="flex flex-row flex-nowrap gap-x-4">
            <Button type="button" variant="secondary" size="icon" disabled={isLoading} onClick={() => refetch()}>
                <RefreshCw />
            </Button>
            <CreateInvite journals_id={journals_id} share_id={share_id}/>
        </div>
        <DataTable columns={columns} data={data ?? []} empty={isLoading ? "Loading..." : "No Invites"}/>
    </div>;
}

interface NewInviteForm {
    amount: number,
    expires_on: {
        enabled: boolean,
        value: Date
    }
}

interface CreateInviteProps {
    journals_id: string,
    share_id: string,
}

function cmp_dates(a: string, b: string) {
    let a_date = new Date(a);
    let b_date = new Date(b);

    if (a_date > b_date) {
        return -1;
    } else if (a_date < b_date) {
        return 1;
    } else {
        return 0;
    }
}

function CreateInvite({journals_id, share_id}: CreateInviteProps) {
    const client = useQueryClient();

    const [view_dialog, set_view_dialog] = useState(false);

    const form = useForm<NewInviteForm>({
        defaultValues: {
            amount: 1,
            expires_on: {
                enabled: false,
                value: new Date(),
            }
        }
    });

    const on_submit: SubmitHandler<NewInviteForm> = async (data, ev) => {
        try {
            let res = await req_api_json<InviteFull[]>("POST", `/journals/${journals_id}/share/${share_id}/invite`, {
                amount: data.amount,
                expires_on: data.expires_on.enabled ? data.expires_on.value.toJSON() : null,
            });

            /*
            client.setQueryData<InviteFull[]>(search_invites_query_key(journals_id, share_id), old => {
                let existing = old ?? [];

                existing.push(...res);

                existing.sort((a, b) => {
                    if (a.status === b.status) {
                        return cmp_dates(a.issued_on, b.issued_on);
                    } else {
                        if (a.status === InviteStatus.Accepted) {
                            return -1;
                        } else if (b.status === InviteStatus.Accepted) {
                            return 1;
                        } else if (a.status === InviteStatus.Rejected) {
                            return -1;
                        } else if (b.status === InviteStatus.Rejected) {
                            return 1;
                        } else {
                            return cmp_dates(a.issued_on, b.issued_on);
                        }
                    }
                });

                return existing;
            });
            */
            client.refetchQueries({
                queryKey: search_invites_query_key(journals_id, share_id),
                exact: true
            });

            form.reset();

            set_view_dialog(false);
        } catch (err) {
            if (err instanceof ApiError) {
                switch (err.kind) {
                    case "InvalidAmount":
                        form.setError("amount", {
                            message: "Must be greater than 0 and less than or equal 10."
                        });
                        break;
                    case "InvalidExpiresOn":
                        form.setError("expires_on.value", {
                            message: "Must be greater than today."
                        });
                        break;
                    case "ShareNotFound":
                        form.setError("root", {
                            message: "The requested share was not found.",
                        });
                        break;
                    case "JournalNotFound":
                        form.setError("root", {
                            message: "the requested journal was not found."
                        });
                        break;
                    default:
                        console.error("failed to create new invites", err);

                        form.setError("root", {
                            message: "Failed to create new invites."
                        });
                        break;
                }
            } else {
                console.error("client error when creating new invites", err);

                form.setError("root", {
                    message: "Client error when creating new invites."
                });
            }
        }
    };

    return <Dialog open={view_dialog} onOpenChange={open => {
        set_view_dialog(open);
    }}>
        <DialogTrigger asChild>
            <Button type="button" variant="secondary">
                <Plus/>New Invite
            </Button>
        </DialogTrigger>
        <DialogContent>
            <FormProvider<NewInviteForm> {...form} children={
                <form className="space-y-4" onSubmit={form.handleSubmit(on_submit)}>
                    <DialogHeader>
                        <DialogTitle>New Invite</DialogTitle>
                        <DialogDescription>
                            Create a new invite to share with other users.
                        </DialogDescription>
                    </DialogHeader>
                    <FormRootError/>
                    <div className="grid grid-cols-4 gap-4">
                        <FormField control={form.control} name="amount" render={({field}) => {
                            return <FormItem>
                                <FormLabel>Amount</FormLabel>
                                <FormControl>
                                    <Input type="number" min={1} max={10} {...field}/>
                                </FormControl>
                                <FormMessage/>
                            </FormItem>
                        }}/>
                        <div className="col-span-3 space-y-2">
                            <FormField control={form.control} name="expires_on.enabled" render={({field}) => {
                                return <FormItem className="flex flex-row flex-nowrap items-center gap-x-2 space-y-0 pt-2">
                                    <FormControl>
                                        <Checkbox
                                            checked={field.value ?? false}
                                            disabled={field.disabled}
                                            onCheckedChange={() => {
                                                field.onChange(!field.value);
                                            }}
                                        />
                                    </FormControl>
                                    <FormLabel className="my-0">Expires On</FormLabel>
                                </FormItem>;
                            }}/>
                            <FormField control={form.control} name={"expires_on.value"} render={({field}) => {
                                let now = new Date();
                                let enabled = form.getValues("expires_on.enabled");

                                return <FormItem className="space-y-0">
                                    <Popover>
                                        <PopoverTrigger asChild>
                                            <FormControl>
                                                <Button
                                                    variant="outline"
                                                    className={"w-[280px] justify-start text-left front-normal"}
                                                    disabled={!enabled}
                                                >
                                                    {format(field.value, "yyyy/MM/dd")}
                                                    <CalendarIcon className="mr-2 h-4 w-4"/>
                                                </Button>
                                            </FormControl>
                                        </PopoverTrigger>
                                        <PopoverContent className="w-auto p-0" align="start">
                                            <Calendar
                                                mode="single"
                                                selected={field.value}
                                                onSelect={field.onChange}
                                                disabled={(date) => {
                                                    return date < now;
                                                }}
                                            />
                                        </PopoverContent>
                                    </Popover>
                                    <FormMessage/>
                                </FormItem>;
                            }}/>
                        </div>
                    </div>
                    <DialogFooter>
                        <Button type="submit">Create</Button>
                    </DialogFooter>
                </form>
            }/>
        </DialogContent>
    </Dialog>
}