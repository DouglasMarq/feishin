import { useMemo } from 'react';

import { useDefaultItemListControls } from '/@/renderer/components/item-list/helpers/item-list-controls';
import { DefaultItemControlProps, ItemControls } from '/@/renderer/components/item-list/types';
import { usePlayer } from '/@/renderer/features/player/context/player-context';
import { usePlayerStore } from '/@/renderer/store';
import { LibraryItem, SongListQuery } from '/@/shared/types/domain-types';
import { Play } from '/@/shared/types/types';

export const useSongListPlayControls = (
    query: Partial<SongListQuery>,
    serverId: string,
): Partial<ItemControls> => {
    const player = usePlayer();
    const baseControls = useDefaultItemListControls();

    return useMemo(
        () => ({
            onDoubleClick: (props: DefaultItemControlProps) => {
                const { internalState, item, itemType, meta } = props;
                const playType = (meta?.playType as Play) || Play.NOW;

                if (!item || itemType !== LibraryItem.SONG || playType !== Play.NOW) {
                    baseControls.onDoubleClick?.(props);
                    return;
                }

                internalState?.setSelected([item]);

                const isCurrentSong = usePlayerStore.getState().getCurrentSong()?.id === item.id;

                player
                    .addToQueueByListQuery(serverId, query, LibraryItem.SONG, Play.NOW, {
                        playSongId: item.id,
                        skipConfirmation: true,
                    })
                    .then(() => {
                        if (isCurrentSong) {
                            player.mediaSeekToTimestamp(0);
                        }
                    });
            },
        }),
        [baseControls, player, query, serverId],
    );
};
