// Übernommen aus Lifeline Hub 3eb9115f2 (16.10.2026): frontend/src/components/instrument/Datenraster.tsx
import type { CSSProperties, ReactNode } from 'react';
import Augenbraue from './Augenbraue';
import { monoStil, useRollen } from './rollenwerte';
import { datenfeldStil, datenrasterStil } from './datenrasterStil';

/**
 * Datenraster — die Detail-Optik statt antds `Descriptions`: je Feld eine Augenbraue über dem
 * Wert, 2–4 Spalten im Fugenraster, Kennungen/Zahlen/Zeiten in Mono.
 *
 * SEMANTIK: ein `<dl>`; jedes Feld ist ein `<div>` mit `<dt>` (Augenbraue) und `<dd>` — die
 * zulässige Gruppierung nach HTML, Vorleser hören Begriff und Wert als Paar.
 *
 * BEARBEITEN AN ORT UND STELLE: der Wert ist ein beliebiger Knoten. Eine Detailseite, die im
 * Edit-Modus ein `Form.Item noStyle` in die Zelle legt, behält dasselbe Raster. `mono` setzt die
 * Schrift am `<dd>`; ein antd-Eingabefeld darin bringt seine eigene mit.
 */
interface DatenrasterProps {
  children: ReactNode;
  /** Höchstzahl der Spalten; das Raster gibt auf schmalem Schirm selbst ab. Vorgabe 3. */
  spalten?: number;
  /** Zugänglicher Name, wenn die Seite mehrere Raster trägt. */
  beschriftung?: string;
  style?: CSSProperties;
}

export default function Datenraster({
  children,
  spalten = 3,
  beschriftung,
  style,
}: DatenrasterProps) {
  const { rollen } = useRollen();
  return (
    <dl
      aria-label={beschriftung}
      data-lfh="datenraster"
      data-fugenraster=""
      style={{ ...datenrasterStil(rollen, spalten), ...style }}
    >
      {children}
    </dl>
  );
}

interface DatenfeldProps {
  /** Begriff — erscheint als Augenbraue. */
  label: ReactNode;
  children: ReactNode;
  /** Wert in Mono (Kennung, Zahl, Zeit, Funkrufname). */
  mono?: boolean;
  /** Über alle Spalten (Freitext, Bearbeitungsfeld). */
  breit?: boolean;
}

export function Datenfeld({ label, children, mono = false, breit = false }: DatenfeldProps) {
  const { token, rollen } = useRollen();
  return (
    <div data-lfh="datenfeld" style={datenfeldStil(rollen, token, breit)}>
      <Augenbraue als="dt">{label}</Augenbraue>
      <dd
        style={{
          margin: 0,
          minWidth: 0,
          overflowWrap: 'anywhere',
          color: rollen.text,
          ...(mono ? monoStil(13) : { fontSize: 13 }),
        }}
      >
        {children}
      </dd>
    </div>
  );
}
