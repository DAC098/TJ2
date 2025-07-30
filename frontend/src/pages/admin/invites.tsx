import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { format, formatDistanceToNow } from "date-fns";
import { Plus, Trash, RefreshCw, Search, CalendarIcon, Copy, CircleDashed, CircleCheck, CircleX } from "lucide-react";
import { useState } from "react";
import { useForm, FormProvider, SubmitHandler } from "react-hook-form";
import { Link } from "react-router-dom";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import { Calendar } from "@/components/ui/calendar";
import { Checkbox } from "@/components/ui/checkbox";
import {
    FormControl,
    FormField,
    FormItem,
    FormLabel,
    FormMessage,
    FormRootError,
} from "@/components/ui/form";
import { Input } from "@/components/ui/input";
import { CenterPage } from "@/components/ui/page";
import {
    Popover,
    PopoverContent,
    PopoverTrigger,
} from "@/components/ui/popover";
import {
    DataTable,
    ColumnDef,
} from "@/components/ui/table";
import { ApiError, req_api_json, req_api_json_empty } from "@/net";
import { send_to_clipboard } from "@/utils";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle, DialogTrigger } from "@/components/ui/dialog";
import { RolePartial } from "./roles";
import { GroupPartial } from "./groups";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { MiniErrorMsg } from "@/components/error";
import { Badge } from "@/components/ui/badge";

enum InviteStatus {
    Pending = 0,
    Accepted = 1,
    Rejected = 2,
}

interface InviteFull {
    token: string,
    name: string,
    issued_on: string,
    expires_on: string | null,
    status: InviteStatus,
    user: InviteUser | null,
    role: InviteRole | null,
    group: InviteGroup | null,
}

interface InviteUser {
    id: number,
    username: string,
}

interface InviteRole {
    id: number,
    name: string,
}

interface InviteGroup {
    id: number,
    name: string,
}

interface InviteTableProps {}

function search_invites_query_key(): ["invites_search"] {
    return ["invites_search"];
}

export function InviteTable({}: InviteTableProps) {
    const {data, isLoading, refetch} = useQuery({
        queryKey: search_invites_query_key(),
        queryFn: async ({queryKey}) => {
            return await req_api_json<InviteFull[]>("GET", `/admin/invites`);
        }
    });

    const {mutate: delete_invite} = useMutation<void, Error, {token: string}>({
        mutationFn: async ({token}) => {
            await req_api_json_empty("DELETE", "/admin/invites", {
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
            cell: ({row}) => {
                return <Button type="button" variant="ghost" size="sm" onClick={() => {
                    send_to_clipboard(row.original.token).then(() => {
                        toast("Copied to clipboard");
                    }).catch(err => {
                        console.error(err);
                        toast(`Failed copying to clipboard: ${row.original.token}`);
                    });
                }}>
                    <pre>{row.original.token}</pre>
                    <Copy/>
                </Button>;
            }
        },
        {
            header: "Status",
            cell: ({row}) => {
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
                    return <Link to={`/admin/users/${row.original.user.id}`}>
                        {row.original.user.username}
                    </Link>;
                }
            }
        },
        {
            header: "Issued On",
            cell: ({row}) => {
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
            cell: ({row}) => {
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
            header: "Role / Group",
            cell: ({row}) => {
                return <div className="flex flex-row items-center gap-2">
                    {row.original.role != null ?
                        <Link to={`/admin/roles/${row.original.role.id}`}>
                            <Badge variant="outline">role: {row.original.role.name}</Badge>
                        </Link>
                        :
                        null
                    }
                    {row.original.group != null ?
                        <Link to={`/admin/groups/${row.original.group.id}`}>
                            <Badge variant="outline">group: {row.original.group.name}</Badge>
                        </Link>
                        :
                        null
                    }
                </div>
            }
        },
        {
            id: "actions",
            cell: ({ row }) => {
                return <div className="w-full flex flex-row justify-end">
                    <Button type="button" variant="destructive" size="icon" onClick={() => {
                        delete_invite({token: row.original.token});
                    }}>
                        <Trash/>
                    </Button>
                </div>
            }
        }
    ];

    return <CenterPage className="pt-4 max-w-6xl">
        <div className="flex flex-row flex-nowrap gap-x-4">
            <div className="w-1/2 relative">
                <Input
                    type="text"
                    placeholder="Search"
                    className="pr-10"
                    disabled={isLoading}
                />
                <Button
                    type="button"
                    variant="ghost"
                    size="icon"
                    className="absolute right-0 top-0"
                    disabled={isLoading}
                >
                    <Search/>
                </Button>
            </div>
            <Button
                type="button"
                variant="secondary"
                size="icon"
                disabled={isLoading}
                onClick={() => refetch()}>
                <RefreshCw />
            </Button>
            <CreateInvite/>
        </div>
        <DataTable columns={columns} data={data ?? []} empty={isLoading ? "Loading..." : "No Invites"}/>
    </CenterPage>;
}

interface NewInviteForm {
    amount: number,
    expires_on: {
        enabled: boolean,
        value: Date
    },
    roles_id: number,
    groups_id: number,
}

interface CreateInviteProps {}

function CreateInvite({}: CreateInviteProps) {
    const client = useQueryClient();

    const [view_dialog, set_view_dialog] = useState(false);

    const form = useForm<NewInviteForm>({
        defaultValues: {
            amount: 1,
            expires_on: {
                enabled: false,
                value: new Date(),
            },
            roles_id: 0,
            groups_id: 0,
        }
    });

    const {data: roles, isLoading: roles_loading, error: roles_error} = useQuery({
        queryKey: ["new_invite_roles_avail"],
        queryFn: async () => {
            return await req_api_json<RolePartial[]>("GET", "/admin/roles");
        }
    });

    const {data: groups, isLoading: groups_loading, error: groups_error} = useQuery({
        queryKey: ["new_invite_groups_avail"],
        queryFn: async () => {
            return await req_api_json<GroupPartial[]>("GET", "/admin/groups");
        }
    });

    const on_submit: SubmitHandler<NewInviteForm> = async (data, ev) => {
        try {
            let res = await req_api_json<InviteFull[]>("POST", `/admin/invites`, {
                amount: data.amount,
                expires_on: data.expires_on.enabled ? data.expires_on.value.toJSON() : null,
                role_id: data.roles_id === 0 ? null : data.roles_id,
                groups_id: data.groups_id === 0 ? null : data.groups_id,
            });

            client.refetchQueries({
                queryKey: search_invites_query_key(),
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
                    case "RoleNotFound":
                        form.setError("roles_id", {
                            message: "Role was not found"
                        });
                        break;
                    case "GroupNotFound":
                        form.setError("groups_id", {
                            message: "Group was not found"
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

    let role_select;

    if (roles_error != null) {
        if (roles_error instanceof ApiError) {
            switch (roles_error.kind) {
                case "PermissionDenied":
                    role_select = null;
                    break;
                default:
                    role_select = <MiniErrorMsg title="ApiError" message={`Failed to load roles: ${roles_error.kind}`}/>;
                    break;
            }
        } else {
            role_select = <MiniErrorMsg title="ClientError" message="Failed to load roles"/>;
        }
    } else if (roles_loading || roles == null) {
        role_select = null;
    } else {
        role_select = <FormField control={form.control} name="roles_id" render={({field}) => {
            return <FormItem className="col-span-2">
                <FormLabel>Role</FormLabel>
                <Select onValueChange={v => field.onChange(parseInt(v, 10))} defaultValue={field.value.toString()}>
                    <FormControl>
                        <SelectTrigger>
                            <SelectValue/>
                        </SelectTrigger>
                    </FormControl>
                    <SelectContent>
                        <SelectItem value="0">No Role</SelectItem>
                        {roles.map(value => {
                            return <SelectItem key={value.id} value={value.id.toString()}>
                                {value.name}
                            </SelectItem>;
                        })}
                    </SelectContent>
                </Select>
                <FormMessage/>
            </FormItem>
        }}/>;
    }

    let group_select;

    if (groups_error != null) {
        if (groups_error instanceof ApiError) {
            switch (groups_error.kind) {
                case "PermissionDenied":
                    group_select = null;
                    break;
                default:
                    group_select = <MiniErrorMsg title="ApiError" message={`Failed to load groups: ${groups_error.kind}`}/>;
                    break;
            }
        } else {
            group_select = <MiniErrorMsg title="ClientError" message="Failed to load groups"/>;
        }
    } else if (groups_loading || groups == null) {
        group_select = null;
    } else {
        group_select = <FormField control={form.control} name="groups_id" render={({field}) => {
            return <FormItem className="col-span-2">
                <FormLabel>Group</FormLabel>
                <Select onValueChange={v => field.onChange(parseInt(v, 10))} defaultValue={field.value.toString()}>
                    <FormControl>
                        <SelectTrigger>
                            <SelectValue/>
                        </SelectTrigger>
                    </FormControl>
                    <SelectContent>
                        <SelectItem value="0">No Group</SelectItem>
                        {groups.map(value => {
                            return <SelectItem key={value.id} value={value.id.toString()}>
                                {value.name}
                            </SelectItem>;
                        })}
                    </SelectContent>
                </Select>
                <FormMessage/>
            </FormItem>
        }}/>;
    }

    return <Dialog open={view_dialog} onOpenChange={open => {
        if (!open) {
            form.reset();
        }

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
                    <div className="grid grid-cols-4 gap-x-4 gap-y-2">
                        <FormField control={form.control} name="amount" render={({field}) => {
                            return <FormItem>
                                <FormLabel>Amount</FormLabel>
                                <FormControl>
                                    <Input
                                        type="number"
                                        min={1}
                                        max={10}
                                        {...field}
                                        onChange={e => field.onChange(parseInt(e.target.value, 10))}
                                    />
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
                        {role_select}
                        {group_select}
                    </div>
                    <DialogFooter>
                        <Button type="submit">Create</Button>
                    </DialogFooter>
                </form>
            }/>
        </DialogContent>
    </Dialog>
}
