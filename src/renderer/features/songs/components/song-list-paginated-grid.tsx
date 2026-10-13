import { UseSuspenseQueryOptions } from '@tanstack/react-query';

import { api } from '/@/renderer/api';
import { useItemListPaginatedLoader } from '/@/renderer/components/item-list/helpers/item-list-paginated-loader';
import { useGridRows } from '/@/renderer/components/item-list/helpers/use-grid-rows';
import { ItemGridList } from '/@/renderer/components/item-list/item-grid-list/item-grid-list';
import { ItemListWithPagination } from '/@/renderer/components/item-list/item-list-pagination/item-list-pagination';
import { useItemListPagination } from '/@/renderer/components/item-list/item-list-pagination/use-item-list-pagination';
import { ItemListGridComponentProps } from '/@/renderer/components/item-list/types';
import { useListContext } from '/@/renderer/context/list-context';
import { songsQueries } from '/@/renderer/features/songs/api/songs-api';
import { useSongListPlayControls } from '/@/renderer/features/songs/hooks/use-song-list-play-controls';
import { useGeneralSettings } from '/@/renderer/store';
import { LibraryItem, SongListQuery, SongListSort, SortOrder } from '/@/shared/types/domain-types';
import { ItemListKey } from '/@/shared/types/types';

interface SongListPaginatedGridProps extends ItemListGridComponentProps<SongListQuery> {}

export const SongListPaginatedGrid = ({
    gap = 'md',
    itemsPerPage = 100,
    itemsPerRow,
    query = {
        sortBy: SongListSort.NAME,
        sortOrder: SortOrder.ASC,
    },
    serverId,
    size,
}: SongListPaginatedGridProps) => {
    const { pageKey } = useListContext();
    const { currentPage, onChange } = useItemListPagination();

    const listCountQuery = songsQueries.listCount({
        query: { ...query, limit: itemsPerPage },
        serverId: serverId,
    }) as UseSuspenseQueryOptions<number, Error, number, readonly unknown[]>;

    const listQueryFn = api.controller.getSongList;

    const { data, pageCount, totalItemCount } = useItemListPaginatedLoader({
        currentPage,
        eventKey: pageKey || ItemListKey.SONG,
        itemsPerPage,
        itemType: LibraryItem.SONG,
        listCountQuery,
        listQueryFn,
        query,
        serverId,
    });

    const rows = useGridRows(LibraryItem.SONG, ItemListKey.SONG, size);
    const { enableGridMultiSelect } = useGeneralSettings();

    const playControls = useSongListPlayControls(query, serverId);

    return (
        <ItemListWithPagination
            currentPage={currentPage}
            itemsPerPage={itemsPerPage}
            onChange={onChange}
            pageCount={pageCount}
            totalItemCount={totalItemCount}
        >
            <ItemGridList
                currentPage={currentPage}
                data={data || []}
                enableMultiSelect={enableGridMultiSelect}
                gap={gap}
                itemsPerRow={itemsPerRow}
                itemType={LibraryItem.SONG}
                overrideControls={playControls}
                rows={rows}
                size={size}
            />
        </ItemListWithPagination>
    );
};
