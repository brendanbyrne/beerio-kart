export interface SimpleItem {
  id: number
  name: string
  image_path: string
}

export interface TrackItem {
  id: number
  name: string
  cup_id: number
  position: number
  image_path: string
}

export interface CupWithTracks {
  id: number
  name: string
  image_path: string
  tracks: TrackItem[]
}

export interface DrinkType {
  id: string
  name: string
  alcoholic: boolean
  created_by: string | null
  created_at: string
}

export interface DrinkTypeInfo {
  id: string
  name: string
  alcoholic: boolean
}

export interface UserPublicProfile {
  id: string
  username: string
  preferred_character_id: number | null
  preferred_body_id: number | null
  preferred_wheel_id: number | null
  preferred_glider_id: number | null
  preferred_drink_type_id: string | null
  created_at: string
}

export interface UserDetailProfile {
  id: string
  username: string
  preferred_character_id: number | null
  preferred_body_id: number | null
  preferred_wheel_id: number | null
  preferred_glider_id: number | null
  preferred_drink_type: DrinkTypeInfo | null
  created_at: string
}

export interface RaceSetup {
  preferred_character_id: number
  preferred_body_id: number
  preferred_wheel_id: number
  preferred_glider_id: number
}

export interface SessionSummary {
  id: string
  host_username: string
  participant_count: number
  race_number: number
  ruleset: string
  last_activity_at: string
}

export interface ParticipantInfo {
  user_id: string
  username: string
  joined_at: string
  left_at: string | null
}

export interface SessionRaceInfo {
  id: string
  race_number: number
  track_id: number
  track_name: string
  cup_name: string
  image_path: string
  created_at: string
}

export interface RaceInfo {
  id: string
  race_number: number
  track_id: number
  track_name: string
  cup_name: string
  run_count: number
  created_at: string
}

export interface SessionDetail {
  id: string
  created_by: string
  host_id: string
  host_username: string
  ruleset: string
  status: string
  created_at: string
  last_activity_at: string
  participants: ParticipantInfo[]
  race_number: number
  current_race: SessionRaceInfo | null
}
