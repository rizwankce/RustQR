package org.rustqr.competitor;

import boofcv.abst.fiducial.QrCodeDetector;
import boofcv.factory.fiducial.ConfigQrCode;
import boofcv.factory.fiducial.FactoryFiducial;
import boofcv.io.image.UtilImageIO;
import boofcv.struct.image.GrayU8;

/**
 * The common adapter protocol writes one UTF-8 payload line per decoded QR.
 * It intentionally does not stringify BoofCV's richer metadata: the harness
 * reports that metadata as unavailable until every adapter can expose it.
 */
public final class BoofcvDecode {
    private BoofcvDecode() {}

    public static void main(String[] args) {
        if (args.length == 1 && args[0].equals("--version")) {
            System.out.println("1.1.7");
            return;
        }
        if (args.length != 1) {
            System.err.println("usage: boofcv_decode.jar <shared.pgm>");
            System.exit(2);
        }
        GrayU8 image = UtilImageIO.loadImage(args[0], GrayU8.class);
        if (image == null) {
            System.err.println("unable to load shared PGM");
            System.exit(2);
        }
        QrCodeDetector<GrayU8> detector = FactoryFiducial.qrcode(new ConfigQrCode(), GrayU8.class);
        detector.process(image);
        int decoded = 0;
        for (var qr : detector.getDetections()) {
            if (qr.message != null) {
                System.out.println(qr.message);
                decoded++;
            }
        }
        if (decoded == 0) System.exit(1);
    }
}
