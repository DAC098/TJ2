import { Routes, Route } from "react-router-dom";

import { CenterPage } from "@/components/ui/page";

import { Entries } from "@/pages/journals/journals_id/entries";
import { Entry } from "@/pages/journals/journals_id/entries/entries_id";
import { Journal } from "@/pages/journals/journals_id";

export function JournalRoutes() {
    return <Routes>
        <Route index element={<JournalsIndex />}/>
        <Route path="/:journals_id" element={<Journal />}/>
        <Route path="/:journals_id/entries" element={<Entries />}/>
        <Route path="/:journals_id/entries/:entries_id" element={<Entry />}/>
    </Routes>;
}

function JournalsIndex() {
    return <CenterPage className="flex items-center justify-center h-full">
        <div className="w-1/2 flex flex-col flex-nowrap items-center">
            <h2 className="text-2xl">Nothing to see here</h2>
            <p>Select a journal on the sidebar to view its entries</p>
        </div>
    </CenterPage>;
}
