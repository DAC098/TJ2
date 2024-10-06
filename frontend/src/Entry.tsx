import { useState, useEffect, JSX } from "react";
import { useParams } from "react-router-dom";

interface JournalEntry {
    id: number,
    users_id: number,
    date: string,
    title: string | null,
    contents: string | null,
    created: string,
    updated: string,
    tags: JournalTag[],
}

interface JournalTag {
    key: string,
    value: string | null,
    created: string,
    updated: string | null,
}

function blank_entry(): JournalEntry {
    let today = new Date();
    let month = (today.getMonth() + 1)
        .toString(10)
        .padStart(2, '0');
    let day = today.getDate()
        .toString(10)
        .padStart(2, '0');

    return {
        id: 0,
        users_id: 0,
        date: `${today.getFullYear()}-${month}-${day}`,
        title: null,
        contents: null,
        created: today.toISOString(),
        updated: null,
        tags: []
    };
}

interface EntrySecProps {
    title: JSX.Element,
    children?: JSX.Element[] | JSX.Element
}

const EntrySec = ({title, children}: EntrySecProps) => {
    return <>
        <div>{title}</div>
        <div>{children}</div>
    </>
};

interface EntrySecTitleProps {
    title: String
}

const EntrySecTitle = ({title}: EntrySecTitleProps) => {
    return <span className="text-right">{title}</span>
};

interface EntryTagProps {
    key?: string | number,
    data: JournalTag
}

const EntryTag = ({key, data}: EntryTagProps) => {
    let [tag_key, setTagKey] = useState(data.key);
    let [tag_value, setTagValue] = useState(data.value ?? "");

    return <div key={key}>
        <input type="text" value={tag_key}/>
        <input type="text" value={tag_value}/>
    </div>
};

const Entry = () => {
    let [entry, setEntry] = useState(blank_entry());
    let [loading, setLoading] = useState(false);

    let { entry_date } = useParams();

    useEffect(() => {
        if (entry_date == null || entry_date == "new") {
            return;
        }

        setLoading(true);

        fetch(`/entries/${entry_date}`).then(res => {
            if (res.status == 404) {
                console.log("failed to find entry");

                return;
            }

            if (res.status !== 200) {
                console.log("non 200 response status:", res);

                return;
            }

            let content_type = res.headers.get("content-type");

            if (content_type == null || content_type !== "application/json") {
                console.log("unspecified content-type from response");

                return;
            }

            if (content_type !== "application/json") {
                console.log("non json content-type");

                return;
            }

            return res.json();
        }).then(json => {
            if (json == null) {
                return;
            }

            setEntry(() => {
                return json as JournalEntry;
            });
        }).catch(err => {
            console.log("failed to retrieve entry:", err);
        }).finally(() => {
            setLoading(false);
        });
    }, []);

    let tags_ele = [];

    for (let tag of entry.tags) {
        tags_ele.push(<EntryTag key={tag.key} data={tag}/>);
    }

    console.log(entry);

    return <div
        className=""
        style={{
            display: "grid",
            gridTemplateColumns: "10rem auto"
        }}
    >
        <EntrySec title={<EntrySecTitle title="Date"/>}>
            <input type="date" value={entry.date}/>
        </EntrySec>
        <EntrySec title={<EntrySecTitle title="Title"/>}>
            <input type="text" value={entry.title ?? ""}/>
        </EntrySec>
        <EntrySec title={<EntrySecTitle title="Contents"/>}>
            <textarea value={entry.contents ?? ""}/>
        </EntrySec>
        <EntrySec title={<EntrySecTitle title="Tags"/>}>
            {tags_ele}
        </EntrySec>
    </div>;
};

export default Entry;
