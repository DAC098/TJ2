import { format } from "date-fns";
import { Plus, Save, Trash, Eye, EyeOff, RefreshCcw, Search, Check } from "lucide-react";
import { useState, useEffect } from "react";
import { useForm, useFieldArray, useFormContext, FormProvider, SubmitHandler,  } from "react-hook-form";
import { Link, useParams, useNavigate } from "react-router-dom";

import { Button } from "@/components/ui/button";
import {
    FormControl,
    FormDescription,
    FormField,
    FormItem,
    FormLabel,
    FormMessage,
} from "@/components/ui/form";
import { Input } from "@/components/ui/input";
import { Separator } from "@/components/ui/separator";
import {
    Sheet,
    SheetContent,
    SheetDescription,
    SheetHeader,
    SheetTitle,
    SheetTrigger,
} from "@/components/ui/sheet";
import {
    DataTable,
    ColumnDef,
} from "@/components/ui/table";
import { GroupList, AttachedGroup } from "@/groups";
import { RoleList, AttachedRole } from "@/roles";

interface UserPartial {
    id: number,
    uid: string,
    username: string,
    created: string,
    updated: string | null
}

async function get_users() {
    let res = await fetch("/admin/users");

    if (res.status !== 200) {
        console.log("non 200 response status:", res);

        return null;
    }

    return await res.json() as UserPartial[];
}

const columns: ColumnDef<UserPartial>[] = [
    {
        accessorKey: "username",
        header: "Username",
        cell: ({ row }) => {
            return <Link to={`/admin/users/${row.original.id}`}>{row.original.username}</Link>;
        }
    },
    {
        accessorKey: "mod",
        header: "Mod",
        cell: ({ row }) => {
            return row.original.updated != null ? row.original.updated : row.original.created;
        }
    }
];

export function Users() {
    let [loading, set_loading] = useState(false);
    let [users, set_users] = useState<UserPartial[]>([]);

    useEffect(() => {
        set_loading(true);

        get_users().then(list => {
            if (list == null) {
                return;
            }

            set_users(list);
        }).catch(err => {
            console.log("failed to load user list");
        }).finally(() => {
            set_loading(false);
        });
    }, []);

    return <div className="max-w-3xl mx-auto my-auto space-y-4">
        <div className="flex flex-row flex-nowrap gap-x-4">
            <Link to="/admin/users/new">
                <Button type="button">New User<Plus/></Button>
            </Link>
        </div>
        <DataTable columns={columns} data={users}/>
    </div>;
};

interface UserForm {
    username: string,
    password: string,
    confirm: string,
    groups: AttachedGroup[],
    roles: AttachedRole[],
}

interface UserFull {
    id: number,
    uid: string,
    username: string,
    created: string,
    updated: string | null,
    groups: AttachedGroup[],
    roles: AttachedRole[],
}

export interface AttachedUser {
    users_id: number,
    username: string,
    added: string,
}

async function get_user(id: string) {
    let res = await fetch(`/admin/users/${id}`);

    if (res.status !== 200) {
        return null;
    }

    return await res.json() as UserFull;
}

interface UserHeaderProps {
    users_id: string | null,
    on_delete: () => void,
}

function UserHeader({users_id, on_delete}: UserHeaderProps) {
    const form = useFormContext<UserForm>();

    return <div className="flex flex-row flex-nowrap gap-x-4 items-center">
        <FormField control={form.control} name="username" render={({field}) => {
            return <FormItem className="w-1/2">
                <FormControl>
                    <Input type="text" placeholder="Username" {...field}/>
                </FormControl>
            </FormItem>
        }}/>
        <Button type="submit">Save<Save/></Button>
        {users_id != null && users_id != "new" ?
            <Button
                type="button"
                variant="destructive"
                onClick={() => {
                    on_delete();
                }}
            >
                Delete
                <Trash/>
            </Button>
            :
            null
        }
    </div>;
}

export function User() {
    const { users_id } = useParams();
    const navigate = useNavigate();

    const [show_password, set_show_password] = useState(false);
    const [loading, set_loading] = useState(false);

    const form = useForm<UserForm>({
        defaultValues: {
            username: "",
            password: "",
            confirm: "",
        }
    });

    const create_user = async (data) => {
        let body = JSON.stringify({
            username: data.username,
            password: data.password,
            confirm: data.confirm,
            groups: data.groups.map(attached => {
                return attached.groups_id;
            }),
            roles: data.roles.map(attached => {
                return attached.role_id;
            })
        });

        let res = await fetch("/admin/users", {
            method: "POST",
            headers: {
                "content-type": "application/json",
                "content-length": body.length.toString(10),
            },
            body
        });

        switch (res.status) {
        case 200:
            return await res.json();
        case 400:
            let json = await res.json();

            console.error("failed to update user", json);
            break;
        case 403:
            console.error("you do not have permission to create users");
            break;
        default:
            console.warn("unhandled response status code");
            break;
        }

        return null;
    };

    const update_user = async (users_id, data) => {
        let body = JSON.stringify({
            username: data.username,
            password: data.password.length !== 0 ? data.password : null,
            groups: data.groups.map(attached => {
                return attached.groups_id;
            }),
            roles: data.roles.map(attached => {
                return attached.role_id;
            })
        });

        let res = await fetch(`/admin/users/${users_id}`, {
            method: "PATCH",
            headers: {
                "content-type": "application/json",
                "content-length": body.length.toString(10),
            },
            body
        });

        switch (res.status) {
        case 200:
            return true;
        case 400:
            let json = await res.json();

            console.error("failed to update user", json);
            break;
        case 403:
            console.error("you do not have permission to update users");
            break;
        case 404:
            console.error("user not found");
            break;
        default:
            console.warn("unhandled response status code");
            break;
        }

        return false;
    };

    const on_submit: SubmitHandler<UserForm> = async (data, event) => {
        data.password = data.password.trim();
        data.confirm = data.confirm.trim();

        if (data.password.length !== 0) {
            if (data.confirm !== data.password) {
                console.warn("confirm does not match password");

                return;
            }
        }

        if (users_id === "new") {
            try {
                let created = await create_user(data);

                if (created == null) {
                    return;
                }

                let form_reset = {
                    username: created.username,
                    password: "",
                    confirm: "",
                    groups: created.groups,
                    roles: created.roles,
                };

                form.reset(form_reset);

                navigate(`/admin/users/${created.id}`);
            } catch (err) {
                console.error("error when creating new user", err);
            }
        } else {
            try {
                if (await update_user(users_id, data)) {
                    data.password = "";
                    data.confirm = "";

                    form.reset(data);
                }
            } catch (err) {
                console.error("error when updating new user", err);
            }
        }
    };

    const delete_user = async () => {
        if (users_id === "new") {
            return;
        }

        try {
            let res = await fetch(`/admin/users/${users_id}`, {
                method: "DELETE"
            });

            switch (res.status) {
            case 200:
                navigate("/admin/users");
                break;
            case 403:
                console.error("you do not have permission to delete users");
                break;
            case 404:
                console.error("user not found");
                break;
            }
        } catch (err) {
            console.error("error when deleting user", err);
        }
    };

    useEffect(() => {
        if (users_id === "new") {
            return;
        }

        set_loading(true);

        get_user(users_id).then(result => {
            let form_reset = {
                username: result.username,
                password: "",
                confirm: "",
                groups: result.groups,
                roles: result.roles,
            };

            form.reset(form_reset);
        }).catch(err => {
            console.error(err);
        }).finally(() => {
            set_loading(false);
        });
    }, []);

    return <div className="max-w-3xl mx-auto my-auto">
        <FormProvider<UserForm> {...form} children={
            <form onSubmit={form.handleSubmit(on_submit)} className="space-y-4">
                <UserHeader users_id={users_id} on_delete={() => {
                    delete_user();
                }}/>
                <Separator />
                <FormField control={form.control} name="password" render={({field}) => {
                    return <FormItem className="w-1/2">
                        <FormLabel>Password</FormLabel>
                        <FormControl>
                            <div className="w-full relative">
                                <Input
                                    type={show_password ? "text" : "password"}
                                    autoComplete="new-password"
                                    className="pr-10"
                                    {...field}
                                />
                                <Button
                                    type="button"
                                    variant="ghost"
                                    size="icon"
                                    className="absolute right-0 top-0"
                                    onClick={() => {
                                        set_show_password(v => (!v));
                                    }}
                                >
                                    {show_password ? <EyeOff/> : <Eye/>}
                                </Button>
                            </div>
                        </FormControl>
                    </FormItem>
                }}/>
                <FormField control={form.control} name="confirm" render={({field}) => {
                    return <FormItem className="w-1/2">
                        <FormLabel>Confirm Password</FormLabel>
                        <FormControl>
                            <Input type="password" {...field}/>
                        </FormControl>
                    </FormItem>
                }}/>
                <Separator />
                <div className="flex flex-row gap-x-4">
                    <GroupList />
                    <RoleList />
                </div>
            </form>
        }/>
    </div>;
}

export function UserList<T>() {
    let form = useFormContext();
    let users = useFieldArray({
        control: form.control,
        name: "users",
    });

    let columns: ColumnDef<AttachedUser>[] = [
        {
            accessorKey: "username",
            header: "Username",
        },
        {
            header: "Added",
            cell: ({ row }) => {
                let date = new Date(row.original.added);

                return format(date, "yyyy/MM/dd");
            }
        },
        {
            id: "drop",
            cell: ({ row }) => {
                return <Button
                    type="button"
                    variant="destructive"
                    size="icon"
                    onClick={() => {
                        users.remove(row.index);
                    }}
                >
                    <Trash/>
                </Button>;
            }
        }
    ];

    return <div className="grow space-y-4 basis-1/2">
        <div className="flex flex-row flex-nowrap gap-x-4 items-center">
            <span>Users</span>
            <AddUsers on_added={new_user => {
                users.append(new_user);
            }}/>
        </div>
        <DataTable columns={columns} data={(users.fields as unknown) as AttachedUser[]}/>
    </div>
}

interface AddUsersProps {
    on_added: (user: AttachedUser) => void,
}

function AddUsers({on_added}: AddUsersProps) {
    let [loading, set_loading] = useState(false);
    let [data, set_data] = useState<UserPartial[]>([]);

    let columns: ColumnDef<UserPartial>[] = [
        {
            accessorKey: "username",
            header: "Username",
        },
        {
            id: "selector",
            cell: ({ row }) => {
                return <Button
                    type="button"
                    variant="ghost"
                    size="icon"
                    onClick={() => {
                        on_added({
                            users_id: row.original.id,
                            username: row.original.username,
                            added: (new Date()).toJSON(),
                        });
                    }}
                >
                    <Plus/>
                </Button>;
            }
        }
    ];

    const retrieve = async () => {
        if (loading) {
            return;
        }

        set_loading(true);

        try {
            let res = await fetch("/admin/users");

            if (res.status === 200) {
                let json = await res.json();

                set_data(json);
            }
        } catch (err) {
            console.error("failed to retrieve users", err);
        }

        set_loading(false);
    };

    return <Sheet onOpenChange={value => {
        if (value) {
            retrieve();
        }
    }}>
        <SheetTrigger asChild>
            <Button type="button" variant="secondary">
                Add Users <Plus/>
            </Button>
        </SheetTrigger>
        <SheetContent>
            <SheetHeader>
                <SheetTitle>Add Users</SheetTitle>
                <SheetDescription>
                    Add users to the selected record
                </SheetDescription>
            </SheetHeader>
            <div className="space-y-4 mt-4">
                <div className="flex flex-row flex-nowrap gap-x-4 items-center">
                    <div className="w-full relative">
                        <Input type="text" placeholder="Search" className="pr-10" disabled={loading}/>
                        <Button
                            type="button"
                            variant="ghost"
                            size="icon"
                            className="absolute right-0 top-0"
                            disabled={loading}
                            onClick={() => {
                                retrieve();
                            }}
                        >
                            <Search/>
                        </Button>
                    </div>
                    <Button type="button" variant="ghost" size="icon" onClick={() => {}}>
                        <RefreshCcw/>
                    </Button>
                </div>
                <DataTable columns={columns} data={data}/>
            </div>
        </SheetContent>
    </Sheet>
}
