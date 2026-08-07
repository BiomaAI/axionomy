import {
  IconAlertTriangle,
  IconApple,
  IconArrowsMove,
  IconBolt,
  IconBox,
  IconBuildingFactory2,
  IconBuildingStore,
  IconChecklist,
  IconCircle,
  IconCloudRain,
  IconCoin,
  IconDoor,
  IconFlag,
  IconGasStation,
  IconInfoCircle,
  IconKey,
  IconLayersSubtract,
  IconMapPin,
  IconPackage,
  IconRadar,
  IconRobot,
  IconRoute,
  IconSettings,
  IconShield,
  IconTemperature,
  IconTool,
  IconTruckDelivery,
  IconUser,
  IconUsers,
} from "@tabler/icons-react";
import type { SceneGlyph } from "./api";

export function SceneIcon({ glyph, size = 22, stroke = 1.8 }: { glyph: SceneGlyph; size?: number; stroke?: number }) {
  const props = { size, stroke, "aria-hidden": true };
  switch (glyph) {
    case "agent": return <IconUser {...props} />;
    case "robot": return <IconRobot {...props} />;
    case "vehicle": return <IconTruckDelivery {...props} />;
    case "package": return <IconPackage {...props} />;
    case "location": return <IconMapPin {...props} />;
    case "goal": return <IconFlag {...props} />;
    case "key": return <IconKey {...props} />;
    case "door": return <IconDoor {...props} />;
    case "fuel": return <IconGasStation {...props} />;
    case "money": return <IconCoin {...props} />;
    case "product": return <IconBox {...props} />;
    case "person": return <IconUser {...props} />;
    case "organization": return <IconBuildingStore {...props} />;
    case "tool": return <IconTool {...props} />;
    case "material": return <IconLayersSubtract {...props} />;
    case "food": return <IconApple {...props} />;
    case "temperature": return <IconTemperature {...props} />;
    case "energy": return <IconBolt {...props} />;
    case "clock": return <IconChecklist {...props} />;
    case "hazard": return <IconAlertTriangle {...props} />;
    case "weather": return <IconCloudRain {...props} />;
    case "repair": return <IconSettings {...props} />;
    case "sensor": return <IconRadar {...props} />;
    case "information": return <IconInfoCircle {...props} />;
    case "move": return <IconArrowsMove {...props} />;
    case "constraint": return <IconRoute {...props} />;
    case "task": return <IconChecklist {...props} />;
    case "machine": return <IconBuildingFactory2 {...props} />;
    case "shield": return <IconShield {...props} />;
    case "token": return <IconCircle {...props} />;
    default: return <IconCircle {...props} />;
  }
}

export function GroupIcon({ size = 22 }: { size?: number }) {
  return <IconUsers size={size} stroke={1.8} aria-hidden="true" />;
}
