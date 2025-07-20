import * as React from "react";
import type { ColumnDef } from "@tanstack/react-table";
import {
    flexRender,
    getCoreRowModel,
    useReactTable,
} from "@tanstack/react-table";

import { cn } from "@/utils";

const Table = React.forwardRef<
  HTMLTableElement,
  React.HTMLAttributes<HTMLTableElement>
>(({ className, ...props }, ref) => (
  <div className="relative w-full overflow-auto">
    <table
      ref={ref}
      className={cn("w-full caption-bottom text-sm", className)}
      {...props}
    />
  </div>
));
Table.displayName = "Table";

const TableHeader = React.forwardRef<
  HTMLTableSectionElement,
  React.HTMLAttributes<HTMLTableSectionElement>
>(({ className, ...props }, ref) => (
  <thead ref={ref} className={cn("[&_tr]:border-b", className)} {...props} />
));
TableHeader.displayName = "TableHeader";

const TableBody = React.forwardRef<
  HTMLTableSectionElement,
  React.HTMLAttributes<HTMLTableSectionElement>
>(({ className, ...props }, ref) => (
  <tbody
    ref={ref}
    className={cn("[&_tr:last-child]:border-0", className)}
    {...props}
  />
));
TableBody.displayName = "TableBody";

const TableFooter = React.forwardRef<
  HTMLTableSectionElement,
  React.HTMLAttributes<HTMLTableSectionElement>
>(({ className, ...props }, ref) => (
  <tfoot
    ref={ref}
    className={cn(
      "border-t bg-muted/50 font-medium [&>tr]:last:border-b-0",
      className
    )}
    {...props}
  />
));
TableFooter.displayName = "TableFooter";

const TableRow = React.forwardRef<
  HTMLTableRowElement,
  React.HTMLAttributes<HTMLTableRowElement>
>(({ className, ...props }, ref) => (
  <tr
    ref={ref}
    className={cn(
      "border-b transition-colors hover:bg-muted/50 data-[state=selected]:bg-muted",
      className
    )}
    {...props}
  />
));
TableRow.displayName = "TableRow";

const TableHead = React.forwardRef<
  HTMLTableCellElement,
  React.ThHTMLAttributes<HTMLTableCellElement>
>(({ className, ...props }, ref) => (
  <th
    ref={ref}
    className={cn(
      "h-12 px-4 text-left align-middle font-medium text-muted-foreground [&:has([role=checkbox])]:pr-0",
      className
    )}
    {...props}
  />
));
TableHead.displayName = "TableHead";

const TableCell = React.forwardRef<
  HTMLTableCellElement,
  React.TdHTMLAttributes<HTMLTableCellElement>
>(({ className, ...props }, ref) => (
  <td
    ref={ref}
    className={cn("p-4 align-middle [&:has([role=checkbox])]:pr-0", className)}
    {...props}
  />
));
TableCell.displayName = "TableCell";

const TableCaption = React.forwardRef<
  HTMLTableCaptionElement,
  React.HTMLAttributes<HTMLTableCaptionElement>
>(({ className, ...props }, ref) => (
  <caption
    ref={ref}
    className={cn("mt-4 text-sm text-muted-foreground", className)}
    {...props}
  />
));
TableCaption.displayName = "TableCaption";


interface DataTableProps<Data, Value> {
    columns: ColumnDef<Data, Value>[],
    data: Data[],
    empty?: React.ReactNode | string
}

const DataTable = <Data, Value>({columns, data, empty}: DataTableProps<Data, Value>) => {
    const table = useReactTable({
        data,
        columns,
        getCoreRowModel: getCoreRowModel(),
    });

    const row_model = table.getRowModel();

    let header_rows = table.getHeaderGroups().map((header_group) => {
        let headers = header_group.headers.map((header) => {
            return <TableHead key={header.id}>
                {header.isPlaceholder ?
                    null
                    :
                    flexRender(
                        header.column.columnDef.header,
                        header.getContext()
                    )
                }
            </TableHead>
        });

        return <TableRow key={header_group.id}>
            {headers}
        </TableRow>
    });

    let body_rows;

    if (row_model.rows.length > 0) {
      body_rows = row_model.rows.map((row) => {
        let cells = row.getVisibleCells().map((cell) => {
            return <TableCell key={cell.id}>
                {flexRender(cell.column.columnDef.cell, cell.getContext())}
            </TableCell>
        });

        return <TableRow key={row.id} data-state={row.getIsSelected() && "selected"}>
            {cells}
        </TableRow>;
      });
    } else if (empty != null) {
      if (typeof empty === "string") {
        body_rows = <TableRow>
          <TableCell colSpan={columns.length} className="h-24 text-center">
              {empty}
          </TableCell>
        </TableRow>;
      } else {
        body_rows = <TableRow>{empty}</TableRow>;
      }
    } else {
      body_rows = <TableRow>
        <TableCell colSpan={columns.length} className="h-24 text-center">
            No Results
        </TableCell>
      </TableRow>;
    }

    return <div className="rounded-md border">
        <Table>
            <TableHeader>
                {header_rows}
            </TableHeader>
            <TableBody>
                {body_rows}
            </TableBody>
        </Table>
    </div>
};

export {
  Table,
  TableHeader,
  TableBody,
  TableFooter,
  TableHead,
  TableRow,
  TableCell,
  TableCaption,
  DataTable,
  ColumnDef,
};
