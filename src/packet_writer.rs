use byteorder::{BigEndian, WriteBytesExt};
use crate::{ManagerType, StandardHeader, NetEvent, NetEventType, EventManifest, EventType, EntityType, EntityManifest, ServerEntityMessage};

const MTU_SIZE: usize = 508 - StandardHeader::bytes_number();

pub struct PacketWriter {
    event_working_bytes: Vec<u8>,
    event_count: u8,
    entity_working_bytes: Vec<u8>,
    entity_message_count: u8,
}

impl PacketWriter {
    /// Construct a new instance of `PacketReader`, the given `buffer` will be used to read information from.
    pub fn new() -> PacketWriter {
        PacketWriter {
            event_working_bytes: Vec::<u8>::new(),
            event_count: 0,
            entity_working_bytes: Vec::<u8>::new(),
            entity_message_count: 0,
        }
    }

    pub fn has_bytes(&self) -> bool {
        return self.event_count != 0 || self.entity_message_count != 0;
    }

    pub fn get_bytes(&mut self) -> Box<[u8]> {

        let mut out_bytes = Vec::<u8>::new();

        //Write manager "header" (manager type & entity count)
        if self.event_count != 0 {
            out_bytes.write_u8(ManagerType::Event as u8).unwrap(); // write manager type //TODO this might be meaningless.. always a fixed order here
            out_bytes.write_u8(self.event_count).unwrap(); // write number of events in the following message
            out_bytes.append(&mut self.event_working_bytes); // write event payload
            self.event_count = 0;
        }

        //Write manager "header" (manager type & entity count)
        if self.entity_message_count != 0 {
            //info!("writing {} entity message, with {} bytes", self.entity_message_count, self.entity_working_bytes.len());
            out_bytes.write_u8(ManagerType::Entity as u8).unwrap(); // write manager type //TODO this might be meaningless.. always a fixed order here
            out_bytes.write_u8(self.entity_message_count).unwrap(); // write number of messages
            out_bytes.append(&mut self.entity_working_bytes); // write event payload

            self.entity_message_count = 0;
        }

        out_bytes.into_boxed_slice()
    }

    fn bytes_number(&self) -> usize {
        return self.event_working_bytes.len() + self.entity_working_bytes.len();
    }

    pub fn write_event<T: EventType>(&mut self, manifest: &EventManifest<T>, event: &Box<dyn NetEvent<T>>) -> bool {
        //Write event payload
        let mut event_payload_bytes = Vec::<u8>::new();
        event.as_ref().write(&mut event_payload_bytes);
        if event_payload_bytes.len() > 255 {
            error!("cannot encode an event with more than 255 bytes, need to implement this");
        }

        //Write event "header" (event id & payload length)
        let mut event_total_bytes = Vec::<u8>::new();

        let type_id = NetEventType::get_type_id(event.as_ref());
        let gaia_id = manifest.get_gaia_id(&type_id); // get gaia id
        event_total_bytes.write_u16::<BigEndian>(gaia_id).unwrap();// write gaia id
        event_total_bytes.write_u8(event_payload_bytes.len() as u8).unwrap(); // write payload length
        event_total_bytes.append(&mut event_payload_bytes); // write payload

        let mut hypothetical_next_payload_size = self.bytes_number() + event_total_bytes.len();
        if self.event_count == 0 {
            hypothetical_next_payload_size += 2;
        }
        if hypothetical_next_payload_size < MTU_SIZE {
            self.event_count += 1;
            self.event_working_bytes.append(&mut event_total_bytes);
            return true;
        } else {
            return false;
        }
    }

    pub fn write_entity_message<T: EntityType>(&mut self, manifest: &EntityManifest<T>, message: &ServerEntityMessage<T>) -> bool {

        let mut entity_total_bytes = Vec::<u8>::new();

        match message {
            ServerEntityMessage::Create(_, local_key, entity) => {

                //write entity payload
                let mut entity_payload_bytes = Vec::<u8>::new();
                entity.as_ref().borrow().write(&mut entity_payload_bytes);
                if entity_payload_bytes.len() > 255 {
                    error!("cannot encode an entity with more than 255 bytes, need to implement this");
                }

                //Write entity "header" (entity id & payload length)
                entity_total_bytes.write_u8(message.write_message_type()).unwrap(); // write entity message type

                let type_id = entity.as_ref().borrow().get_type_id();
                let gaia_id = manifest.get_gaia_id(&type_id); // get gaia id
                entity_total_bytes.write_u16::<BigEndian>(gaia_id).unwrap();// write gaia id
                entity_total_bytes.write_u16::<BigEndian>(*local_key).unwrap();//write local key
                entity_total_bytes.write_u8(entity_payload_bytes.len() as u8).unwrap(); // write payload length
                entity_total_bytes.append(&mut entity_payload_bytes); // write payload
            }
            ServerEntityMessage::Delete(_, local_key) => {

                entity_total_bytes.write_u8(message.write_message_type()).unwrap(); //Write entity message type
                entity_total_bytes.write_u16::<BigEndian>(*local_key).unwrap();//write local key
            }
            ServerEntityMessage::Update(_, local_key, state_mask, entity) => {
                //write entity payload
                let mut entity_payload_bytes = Vec::<u8>::new();
                entity.as_ref().borrow().write_partial(state_mask, &mut entity_payload_bytes);
                if entity_payload_bytes.len() > 255 {
                    error!("cannot encode an entity with more than 255 bytes, need to implement this");
                }

                //Write entity "header" (entity id & payload length)
                entity_total_bytes.write_u8(message.write_message_type()).unwrap(); // write entity message type

                entity_total_bytes.write_u16::<BigEndian>(*local_key).unwrap();//write local key
                state_mask.as_ref().borrow_mut().write(&mut entity_total_bytes);// write state mask
                entity_total_bytes.write_u8(entity_payload_bytes.len() as u8).unwrap(); // write payload length
                entity_total_bytes.append(&mut entity_payload_bytes); // write payload
            }
        }

        let mut hypothetical_next_payload_size = self.bytes_number() + entity_total_bytes.len();
        if self.entity_message_count == 0 {
            hypothetical_next_payload_size += 2;
        }
        if hypothetical_next_payload_size < MTU_SIZE {
            self.entity_message_count += 1;
            self.entity_working_bytes.append(&mut entity_total_bytes);
            return true;
        } else {
            return false;
        }
    }
}