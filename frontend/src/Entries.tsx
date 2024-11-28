import { useState, useEffect } from "react";
import { Link } from "react-router-dom";

import {
    DataTable,
    ColumnDef,
} from "@/components/ui/table";

interface JournalEntry {
    id: number,
    date: string,
    created: string,
    updated: string | null,
    tags: JournalTags
}

interface JournalTags {
    [key: string]: string | null
}

async function retrieve_entries() {
    let res = await fetch("/entries");

    if (res.status !== 200) {
        return null;
    }

    return await res.json() as JournalEntry[];
}

const columns: ColumnDef<JournalEntry>[] = [
    {
        accessorKey: "date",
        header: "Date",
        cell: ({ row }) => {
            let date = row.original.date;

            return <Link to={`/entries/${date}`}>{date}</Link>;
        }
    },
    {
        accessorKey: "title",
        header: "Title",
    },
    {
        accessorKey: "tags",
        header: "Tags",
        cell: ({ row }) => {
            let list = [];

            for (let tag in row.original.tags) {
                list.push(<span key={tag}>{tag}</span>);
            }

            return <>{list}</>;
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

const Entries = () => {
    let [loading, setLoading] = useState(false);
    let [entries, setEntries] = useState<JournalEntry[]>([]);

    useEffect(() => {
        console.log("loading entries");

        setLoading(true);

        retrieve_entries().then(json => {
            setEntries(() => {
                return json;
            });
        }).catch(err => {
            console.error("failed to retrieve entries:", err);
        }).finally(() => {
            setLoading(false);
        });
    }, []);

    if (loading) {
        return <div>loading entries</div>
    } else {
        return <div>
            <DataTable columns={columns} data={entries}/>
        </div>
    }
};

export default Entries;
