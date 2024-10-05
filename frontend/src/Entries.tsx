import { useState, useEffect } from "react";

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

const Entries = () => {
    let [loading, setLoading] = useState(false);
    let [entries, setEntries] = useState<JournalEntry[]>([]);

    useEffect(() => {
        console.log("loading entries");

        setLoading(true);

        fetch("/entries").then(res => {
            if (res.status !== 200) {
                console.log("non 200 response status:", res);

                return;
            }

            let content_type = res.headers.get("content-type");

            if (content_type == null || content_type !== "application/json") {
                console.log("unknown or invalid content-type");

                return;
            }

            return res.json();
        }).then(json => {
            if (json == null) {
                return;
            }

            setEntries(() => {
                return json as JournalEntry[];
            });
        }).catch(err => {
            console.log("failed to retrieve entries:", err);
        }).finally(() => {
            setLoading(false);
        });
    }, []);

    console.log("entries:", entries);

    let entry_rows = [];

    for (let entry of entries) {
        let tags = [];

        for (let tag in entry.tags) {
            tags.push(<span key={tag}>{tag}</span>);
        }

        let mod = entry.updated != null ? entry.updated : entry.created;

        entry_rows.push(<tr key={entry.date}>
            <td>{entry.date}</td>
            <td>{tags}</td>
            <td>{mod}</td>
        </tr>);
    }

    if (loading) {
        return <div>loading entries</div>
    } else {
        return <div>
            <table>
                <thead>
                    <tr className="sticky top-0 bg-white">
                        <th>Date</th>
                        <th>Tags</th>
                        <th>Mod</th>
                    </tr>
                </thead>
                <tbody>{entry_rows}</tbody>
            </table>
        </div>
    }
};

export default Entries;
