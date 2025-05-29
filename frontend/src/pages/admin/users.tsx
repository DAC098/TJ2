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

export function Users() {
    let [loading, set_loading] = useState(false);
    let [users, set_users] = useState<UserPartial[]>([]);

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
