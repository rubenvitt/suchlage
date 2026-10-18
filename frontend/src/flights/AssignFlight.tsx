import { useMutation, useQueryClient } from '@tanstack/react-query';
import { apiSend } from '../api/client';
import { queryKeys, useMe } from '../api/queries';
import { AssignMissionSelect } from '../missions/AssignMissionSelect';

/** „Einsatz zuordnen“ für einen Flug ohne Einsatz; nur für Bedienung und Verwaltung. */
export function AssignFlight({ flightId }: { flightId: number }) {
  const role = useMe().data?.role;
  const client = useQueryClient();
  const assign = useMutation({
    mutationFn: (missionId: number) =>
      apiSend<unknown>('PUT', `/api/flights/${flightId}/mission`, { mission_id: missionId }),
    onSuccess: () =>
      Promise.all([
        client.invalidateQueries({ queryKey: queryKeys.flights }),
        client.invalidateQueries({ queryKey: queryKeys.photos }),
      ]),
  });
  if (role !== 'operator' && role !== 'admin') return null;
  return (
    <>
      <AssignMissionSelect busy={assign.isPending} onAssign={(id) => assign.mutate(id)} />
      {assign.isError && <span role="alert">Zuordnung fehlgeschlagen.</span>}
    </>
  );
}
