import { useQueryClient } from '@tanstack/react-query';
import { Dropdown, type MenuProps } from 'antd';
import { useNavigate } from 'react-router';
import { apiSend } from '../api/client';
import { queryKeys } from '../api/queries';
import type { Me } from '../contract/generated/Me';
import { useThemeMode, type ThemeModus } from '../theme/ThemeModeProvider';

const MODE_LABELS: Record<ThemeModus, string> = { system: 'wie System', light: 'Hell', dark: 'Dunkel' };

const initials = (name: string) =>
  name
    .split(' ')
    .map((part) => part[0] ?? '')
    .join('')
    .slice(0, 2)
    .toUpperCase();

/** Benutzermenü an den Initialen. Draußen bei Sonne braucht es Hell; die Wahl merkt sich `ThemeModeProvider`. */
export function UserMenu({ me }: { me: Me }) {
  const { modus, setModus } = useThemeMode();
  const client = useQueryClient();
  const navigate = useNavigate();

  async function logout() {
    // Auch ohne erreichbaren Server abmelden: die Oberfläche vergisst die Sitzung in jedem Fall.
    await apiSend('POST', '/api/auth/logout').catch(() => {});
    client.setQueryData(queryKeys.me, null);
    navigate('/anmelden', { replace: true });
  }

  const items: MenuProps['items'] = [
    { key: 'name', label: me.display_name, disabled: true },
    { type: 'divider' },
    {
      key: 'darstellung',
      type: 'group',
      label: 'Darstellung',
      children: (Object.keys(MODE_LABELS) as ThemeModus[]).map((m) => ({ key: m, label: MODE_LABELS[m] })),
    },
    { type: 'divider' },
    { key: 'logout', label: 'Abmelden' },
  ];
  const onClick: MenuProps['onClick'] = ({ key }) =>
    key === 'logout' ? void logout() : setModus(key as ThemeModus);

  return (
    <Dropdown
      menu={{ items, onClick, selectable: true, selectedKeys: [modus] }}
      trigger={['click']}
      placement="bottomRight"
    >
      <button type="button" className="sl-user" aria-label={`Benutzermenü von ${me.display_name}`}>
        {initials(me.display_name)}
      </button>
    </Dropdown>
  );
}
