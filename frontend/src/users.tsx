import { format } from "date-fns";
import { Plus, Save, Trash, Eye, EyeOff } from "lucide-react";
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

interface UserPartial {
    id: number,
    uid: string,
    username: string,
    created: string,
    updated: string | null
}

async function get_users() {
    let res = await fetch("/users");

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
            return <Link to={`/users/${row.original.id}`}>{row.original.username}</Link>;
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
            <Link to="/users/new">
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

interface AttachedGroup {
    groups_id: number,
    name: string,
    added: string
}

interface AttachedRole {
    role_id: number,
    name: string,
    added: string,
}

async function get_user(id: string) {
    let res = await fetch(`/users/${id}`);

    if (res.status !== 200) {
        return null;
    }

    return await res.json() as UserFull;
}

interface UserHeaderProps {
    users_id: string | null
}

function UserHeader({users_id}: UserHeaderProps) {
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
            <form className="space-y-4">
                <UserHeader users_id={users_id} />
                <FormField control={form.control} name="password" render={({field}) => {
                    return <FormItem className="w-1/2">
                        <FormLabel>Password</FormLabel>
                        <FormControl>
                            <div className="w-full relative">
                                <Input type={show_password ? "text" : "password"} autocomplete="new-password" {...field}/>
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
                <div className="flex flex-row gap-x-4">
                    <GroupList />
                    <RoleList />
                </div>
            </form>
        }/>
    </div>;
}

function GroupList() {
    let form = useFormContext<UserForm>();
    let groups = useFieldArray<UserForm, "groups">({
        control: form.control,
        name: "groups",
    });

    let columns: ColumnDef<AttachedGroup>[] = [
        {
            accessorKey: "name",
            header: "Name",
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
                        groups.remove(row.index);
                    }}
                >
                    <Trash/>
                </Button>;
            }
        }
    ];

    return <div className="grow space-y-4 basis-1/2">
        <div className="flex flex-row flex-nowrap gap-x-4 items-center">
            <span>Groups</span>
            <AddGroups />
        </div>
        <DataTable columns={columns} data={groups.fields}/>
    </div>
}

function AddGroups() {
    return <Sheet>
        <SheetTrigger asChild>
            <Button type="button" variant="secondary">Add Group<Plus/></Button>
        </SheetTrigger>
        <SheetContent>
            <SheetHeader>
                <SheetTitle>Add Groups</SheetTitle>
                <SheetDescription>
                    Add groups to the selected user
                </SheetDescription>
            </SheetHeader>
            content
        </SheetContent>
    </Sheet>
}

function RoleList() {
    let form = useFormContext<UserForm>();
    let roles = useFieldArray<UserForm, "roles">({
        control: form.control,
        name: "roles",
    });

    let columns: ColumnDef<AttachedRole>[] = [
        {
            accessorKey: "name",
            header: "Name",
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
                    variant="descructive"
                    size="icon"
                    onClick={() => {
                        roles.remove(row.index);
                    }}
                >
                    <Trash/>
                </Button>;
            }
        }
    ];

    return <div className="grow space-y-4 basis-1/2">
        <div className="flex flex-row flex-nowrap gap-x-4 items-center">
            <span>Roles</span>
            <AddRoles />
        </div>
        <DataTable columns={columns} data={roles.fields}/>
    </div>;
}

function AddRoles() {
    return <Sheet>
        <SheetTrigger asChild>
            <Button type="button" variant="secondary">Add Roles<Plus/></Button>
        </SheetTrigger>
        <SheetContent>
            <SheetHeader>
                <SheetTitle>Add Roles</SheetTitle>
                <SheetDescription>
                    Add roles to the selected user
                </SheetDescription>
            </SheetHeader>
            content
        </SheetContent>
    </Sheet>
}
